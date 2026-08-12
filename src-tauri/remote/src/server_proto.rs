use std::time::Duration;

use database::borrow_db_checked;
use http::{
    HeaderValue, Request, Response, StatusCode, Uri,
    header::{CONTENT_SECURITY_POLICY, USER_AGENT, X_FRAME_OPTIONS},
};
use log::{error, warn};
use tauri::UriSchemeResponder;

use crate::{
    error::RemoteAccessError,
    requests::send_with_retry_raw,
    utils::{DROP_CLIENT_ASYNC, SERVER_PROTO_SEM, bounded_bytes},
};

/// Cap for `server://` proxied responses. The proxy is used for arbitrary
/// HTML/JS content from the drop server, so we give it plenty of room
/// (128 MiB) without letting a malicious server drain memory.
const SERVER_PROTO_CAP: u64 = 128 * 1024 * 1024;

/// Deadline for one proxied request.
///
/// Stated here rather than inherited from `DROP_CLIENT_ASYNC` because it is one
/// half of a budget that has to stay nested inside the frontend's
/// `API_TIMEOUT_MS`: this layer must always be the one that gives up first, so
/// the webview receives a real 504 instead of aborting a request that keeps
/// running. Tauri gives a custom-protocol handler no cancellation signal, so an
/// abort on the webview side does not stop this task or release its permit.
const SERVER_PROTO_TIMEOUT: Duration = Duration::from_secs(15);

/// How long a proxied request will wait for a concurrency permit before giving
/// up on the queue and sending anyway.
///
/// The cap exists to be kind to a home NAS, not to hold up a page. Everything
/// here is user-blocking, so when the pool is full the right answer is one extra
/// socket, not an unbounded wait: the alternative is a stalled iframe with no
/// feedback at all. Counts against `SERVER_PROTO_TIMEOUT`'s share of the budget,
/// hence seconds rather than tens of seconds.
const SERVER_PROTO_QUEUE_WAIT: Duration = Duration::from_secs(2);

/// Attempts for a *safe* proxied request.
///
/// One. This proxy carries every mutating call in the app (profile PATCH,
/// showcase PUT, favourites add/remove, request submissions, votes), and
/// `send_with_retry_raw` retries transport timeouts and 5xx for whatever method
/// it is handed. A 15s timeout on a POST the server already applied would
/// silently re-send it, which is exactly the duplicate the frontend refuses to
/// risk by not retrying mutations itself.
///
/// It is also one for GETs, because the frontend already retries those once. Two
/// retry layers whose budgets are not nested is what produced the original
/// failure: the outer deadline fired first, abandoned a task that kept its
/// permit, and queued a second request behind the corpse.
const SERVER_PROTO_SAFE_ATTEMPTS: u32 = 1;

pub async fn handle_server_proto_offline_wrapper(
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    responder.respond(match handle_server_proto_offline(request).await {
        Ok(res) => res,
        Err(status) => {
            error!("Unexpected error in offline proto handler: {}", status);
            Response::builder()
                .status(status)
                .body(Vec::new())
                .unwrap_or_default()
        }
    });
}

pub async fn handle_server_proto_offline(
    _request: Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, StatusCode> {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .unwrap_or_default())
}

pub async fn handle_server_proto_wrapper(request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    match handle_server_proto(request).await {
        Ok(r) => responder.respond(r),
        Err(e) => {
            warn!("server proto error: {e}");
            responder.respond(
                Response::builder()
                    .status(e)
                    .body(Vec::new())
                    .unwrap_or_default(),
            );
        }
    }
}

/// Pick the status the webview should see for a proxy failure.
///
/// A transport error carries no HTTP status at all, so the old
/// `e.status().unwrap_or(BAD_REQUEST)` reported every single timeout as an
/// empty-bodied 400 — which reads as a client bug and surfaced to the user as
/// "API <path> failed: 400 Bad Request". These are all upstream conditions, so
/// they belong in the gateway range.
///
/// Only reached when there is no response at all. A response the server actually
/// produced — including a 5xx — is copied through with its own status, headers
/// and body, so this never masks something the server said.
fn proxy_failure_status(error: &RemoteAccessError) -> StatusCode {
    match error {
        RemoteAccessError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        RemoteAccessError::FetchError(e) if e.is_timeout() => StatusCode::GATEWAY_TIMEOUT,
        RemoteAccessError::FetchErrorLegacy(e) if e.is_timeout() => StatusCode::GATEWAY_TIMEOUT,
        RemoteAccessError::Unauthorized => StatusCode::UNAUTHORIZED,
        // A connect failure is "the upstream did not give me an answer at all".
        _ => StatusCode::BAD_GATEWAY,
    }
}

async fn handle_server_proto(request: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, StatusCode> {
    // Scoped tight: `borrow_db_checked` is a blocking std RwLock read and this
    // runs on a tokio worker, so the guard must not outlive the two fields we
    // need from it and must never be held across an await.
    let (remote_uri, web_token) = {
        let db_handle = borrow_db_checked();
        let auth = match db_handle.auth.as_ref() {
            Some(auth) => auth,
            None => {
                error!("Could not find auth in database");
                return Err(StatusCode::UNAUTHORIZED);
            }
        };
        let web_token = match &auth.web_token {
            Some(token) => token.clone(),
            None => return Err(StatusCode::UNAUTHORIZED),
        };
        let remote_uri = match db_handle.base_url.parse::<Uri>() {
            Ok(uri) => uri,
            Err(e) => {
                error!("Failed to parse base url: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };
        (remote_uri, web_token)
    };

    // Consume the request to move parts instead of cloning
    let (parts, body) = request.into_parts();

    let mut new_uri = parts.uri.into_parts();
    new_uri.authority = remote_uri.authority().cloned();
    new_uri.scheme = remote_uri.scheme().cloned();
    let new_uri = match Uri::from_parts(new_uri) {
        Ok(uri) => uri,
        Err(e) => {
            error!("Failed to build new uri from parts: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let new_uri_string = new_uri.to_string();

    let mut headers = parts.headers;
    headers.remove(USER_AGENT);
    headers.append(USER_AGENT, HeaderValue::from_static("Drop Desktop Client"));
    // Use `insert`, not `append`, for Authorization: if the iframe document
    // somehow already carried an Authorization header we must *replace* it
    // with our web token, never send two (the backend would see an ambiguous
    // pair, and a stale value could leak).
    match HeaderValue::from_str(&format!("Bearer {web_token}")) {
        Ok(mut val) => {
            val.set_sensitive(true);
            headers.insert("Authorization", val);
        }
        Err(e) => {
            error!("Failed to create Authorization header: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // A pool of our own, never shared with decorative images, and with a bound
    // on the wait rather than the unbounded FIFO queue this used to join. The
    // permit is held across the body read so it covers the whole socket
    // lifetime, released on early return and on panic, and nothing in this scope
    // waits on a second permit.
    let _permit = match tokio::time::timeout(SERVER_PROTO_QUEUE_WAIT, SERVER_PROTO_SEM.acquire())
        .await
    {
        Ok(Ok(permit)) => Some(permit),
        Ok(Err(e)) => {
            error!("server:// semaphore closed: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        Err(_) => {
            warn!(
                "server:// queue still full after {SERVER_PROTO_QUEUE_WAIT:?}; sending {new_uri_string} unguarded"
            );
            None
        }
    };

    // `build_request` runs once per attempt, so the method, headers and body
    // are cloned rather than moved. Bodies through this proxy are page loads
    // and small JSON posts, not uploads.
    let method = parts.method;
    // Mutating methods never get a second attempt - see SERVER_PROTO_SAFE_ATTEMPTS.
    let attempts = if method.is_safe() {
        SERVER_PROTO_SAFE_ATTEMPTS
    } else {
        1
    };
    let response = match send_with_retry_raw(&method, &new_uri_string, attempts, || {
        DROP_CLIENT_ASYNC
            .request(method.clone(), &new_uri_string)
            .timeout(SERVER_PROTO_TIMEOUT)
            .headers(headers.clone())
            .body(body.clone())
    })
    .await
    {
        Ok(response) => response,
        Err(e) => {
            warn!("server:// proxy request failed: {e}");
            return Err(proxy_failure_status(&e));
        }
    };

    let response_status = response.status();
    let mut client_http_response = Response::builder()
        .status(response_status)
        .header("Access-Control-Allow-Origin", "*");

    if let Some(client_response_headers) = client_http_response.headers_mut() {
        for (header, header_value) in response.headers() {
            if header == CONTENT_SECURITY_POLICY  {
                continue;
            }
            if header == X_FRAME_OPTIONS {
                continue;
            }
            client_response_headers.insert(header, header_value.clone());
        }
    };

    let response_body = match bounded_bytes(response, SERVER_PROTO_CAP).await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!("server:// proxy rejected oversized body: {e}");
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
    };

    let client_http_response = match client_http_response.body(response_body) {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to build server proto response: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(client_http_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeout_is_a_gateway_timeout_not_a_bad_request() {
        assert_eq!(
            proxy_failure_status(&RemoteAccessError::Timeout),
            StatusCode::GATEWAY_TIMEOUT
        );
    }

    #[test]
    fn an_unreachable_server_is_a_bad_gateway() {
        assert_eq!(
            proxy_failure_status(&RemoteAccessError::ServerUnavailable("HTTP 503".into())),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn a_rejected_token_still_reads_as_unauthorized() {
        assert_eq!(
            proxy_failure_status(&RemoteAccessError::Unauthorized),
            StatusCode::UNAUTHORIZED
        );
    }
}

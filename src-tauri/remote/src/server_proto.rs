use database::borrow_db_checked;
use http::{
    HeaderMap, HeaderValue, Request, Response, StatusCode, Uri, header::{CONTENT_SECURITY_POLICY, USER_AGENT, X_FRAME_OPTIONS},
};
use log::{error, warn};
use tauri::UriSchemeResponder;

use crate::utils::DROP_CLIENT_ASYNC;

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

async fn handle_server_proto(request: Request<Vec<u8>>) -> Result<Response<Vec<u8>>, StatusCode> {
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
    match HeaderValue::from_str(&format!("Bearer {web_token}")) {
        Ok(val) => { headers.append("Authorization", val); }
        Err(e) => {
            error!("Failed to create Authorization header: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let response = match DROP_CLIENT_ASYNC
        .request(parts.method, new_uri_string)
        .headers(headers)
        .body(body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            warn!("Could not send response. Got {e:?} when sending");
            return Err(e.status().unwrap_or(StatusCode::BAD_REQUEST));
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

    let response_body = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => return Err(e.status().unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)),
    };

    let client_http_response = match client_http_response.body(response_body.to_vec()) {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to build server proto response: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(client_http_response)
}

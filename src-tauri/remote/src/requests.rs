//! The single HTTP entry point for every Drop-server call in the `remote`
//! crate.
//!
//! # Why one helper
//!
//! Before this module each feature hand-rolled its own `DROP_CLIENT_ASYNC
//! .get(..).header(..).send()` and invented its own (usually absent) retry
//! story. That meant a 503 during a save-sync would hard-fail while the same
//! 503 during playtime-start was retried — inconsistent and hard to reason
//! about. Every request now flows through [`remote_request`] (typed) or the
//! thin [`make_authenticated_get`] / [`make_authenticated_post`] shims, all of
//! which share:
//!
//! * **Auth** — a fresh ES384 JWT minted *per attempt* via
//!   [`generate_authorization_header`]. The token has a 10-second TTL, so
//!   minting it inside the retry loop guarantees a retry never carries an
//!   expired token. There is deliberately no long-lived bearer token to
//!   refresh — see `docs/audit/remote-comms-2026.md`.
//! * **Retry + backoff** — transient failures (connect/timeout errors, HTTP
//!   429 and 5xx) are retried with exponential backoff + jitter. 4xx (other
//!   than 429) and parse errors fail fast.
//! * **Timeouts** — inherited from `DROP_CLIENT_ASYNC` (5s connect, 15s total).
//! * **Loud logging** — every attempt, retry and give-up is logged with a
//!   `[REQ]` tag so the request lifecycle is greppable.

use std::time::Duration;

use database::DB;
use log::{debug, info, warn};
use reqwest::Method;
use reqwest_middleware::Error as MiddlewareError;
use serde::de::DeserializeOwned;
use url::Url;

use crate::{
    auth::generate_authorization_header,
    error::RemoteAccessError,
    utils::{bounded_json, DROP_CLIENT_ASYNC, DEFAULT_JSON_CAP_BYTES},
};

/// Default number of attempts (1 initial + retries) for a transient-failure
/// request. Three keeps a flaky-wifi blip recoverable without making a genuine
/// outage feel like a hang.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 3;

/// Base delay for exponential backoff. Attempt N waits `BASE * 2^(N-1)` plus
/// jitter, i.e. ~0.5s, ~1s, ~2s.
const BACKOFF_BASE: Duration = Duration::from_millis(500);

/// Build a Drop API URL from path components + query pairs.
///
/// Path components are joined with `/` and resolved against the configured
/// base URL. Unchanged behaviour — kept here so it stays beside the request
/// helpers that consume it.
pub fn generate_url(
    path_components: &[&str],
    query: &[(&str, &str)],
) -> Result<Url, RemoteAccessError> {
    let path_appended = path_components.join("/");
    let mut base_url = DB.fetch_base_url().join(&path_appended)?;
    {
        let mut queries = base_url.query_pairs_mut();
        for (param, val) in query {
            queries.append_pair(param.as_ref(), val.as_ref());
        }
    }
    Ok(base_url)
}

/// Small deterministic jitter (0–250ms) derived from the system clock so a
/// fleet of clients retrying after a server blip don't all hit the same
/// instant. Avoids pulling in the `rand` crate for something this minor.
fn backoff_jitter() -> Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    Duration::from_millis(u64::from(nanos % 250))
}

/// Compute the backoff delay before `attempt` (1-indexed; attempt 1 = no wait).
fn backoff_for(attempt: u32) -> Duration {
    BACKOFF_BASE * 2u32.pow(attempt.saturating_sub(1)) + backoff_jitter()
}

/// The single retry-aware request core. Mints a fresh JWT per attempt, sends
/// the request, and retries transient failures with exponential backoff.
///
/// `build_request` is invoked once per attempt to produce a fresh
/// `RequestBuilder` (it must be a closure, not a prebuilt builder, because
/// `reqwest_middleware::RequestBuilder` is not `Clone` and the JSON body would
/// otherwise be consumed by the first `.send()`). The auth header is added
/// here so callers never have to think about it.
///
/// Returns the raw [`reqwest::Response`] on the first 2xx (or first non-2xx
/// that is *not* worth retrying — the caller inspects the status). Only
/// transport errors and 5xx/429 trigger a retry.
async fn send_with_retry<F>(
    method: &Method,
    url: &Url,
    max_attempts: u32,
    build_request: F,
) -> Result<reqwest::Response, RemoteAccessError>
where
    F: Fn() -> reqwest_middleware::RequestBuilder,
{
    let max_attempts = max_attempts.max(1);
    let mut last_err: Option<RemoteAccessError> = None;

    for attempt in 1..=max_attempts {
        if attempt > 1 {
            let delay = backoff_for(attempt);
            info!(
                "[REQ] retry {attempt}/{max_attempts} for {method} {url} after {delay:?}"
            );
            tokio::time::sleep(delay).await;
        } else {
            debug!("[REQ] {method} {url} (attempt 1/{max_attempts})");
        }

        // Fresh auth header every attempt — the JWT lives only 10s.
        let builder = build_request().header("Authorization", generate_authorization_header());

        match builder.send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_server_error() || status.as_u16() == 429 {
                    // Transient server-side condition — worth another attempt.
                    warn!(
                        "[REQ] {method} {url} returned {status} (attempt {attempt}/{max_attempts})"
                    );
                    last_err = Some(RemoteAccessError::ServerUnavailable(format!(
                        "HTTP {status}"
                    )));
                    continue;
                }
                // 2xx or a non-retryable 4xx — hand back to the caller, which
                // decides how to interpret the status.
                if attempt > 1 {
                    info!("[REQ] {method} {url} succeeded on attempt {attempt}");
                }
                return Ok(response);
            }
            Err(e) => {
                let wrapped = RemoteAccessError::from(e);
                if wrapped.is_retryable() && attempt < max_attempts {
                    warn!(
                        "[REQ] {method} {url} transport error (attempt {attempt}/{max_attempts}): {wrapped}"
                    );
                    last_err = Some(wrapped);
                    continue;
                }
                warn!("[REQ] {method} {url} failed permanently: {wrapped}");
                return Err(wrapped);
            }
        }
    }

    Err(last_err.unwrap_or(RemoteAccessError::Timeout))
}

/// Classify a non-2xx response into an actionable [`RemoteAccessError`].
/// Reads the body (bounded) so the error message carries server context.
async fn classify_error_response(response: reqwest::Response) -> RemoteAccessError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        warn!("[REQ] auth rejected by server ({status}): {body}");
        return RemoteAccessError::Unauthorized;
    }
    if status.is_server_error() {
        return RemoteAccessError::ServerUnavailable(format!("HTTP {status}: {body}"));
    }
    RemoteAccessError::ServerError {
        status: status.as_u16(),
        message: if body.is_empty() {
            status.to_string()
        } else {
            body
        },
    }
}

/// Marks a request that carries no body — the default type parameter `B` on
/// [`RemoteRequest`] for GET requests. It derives `Serialize` only to satisfy
/// the `B: Serialize` bound; the GET code path never actually serializes a
/// body, so the impl is never invoked on the wire.
#[derive(serde::Serialize)]
pub struct NoBody;

/// A description of one Drop API call: method, URL, optional JSON body, retry
/// budget and response-size cap. Build with [`RemoteRequest::get`] /
/// [`RemoteRequest::post`] and execute via [`remote_request`] (typed response)
/// or [`RemoteRequest::send_raw`] (raw response).
///
/// The body is held by reference and re-serialized by `reqwest` on each retry
/// attempt — no `serde_json` round-trip here, callers keep passing their inner
/// `#[derive(Serialize)]` structs.
pub struct RemoteRequest<'a, B: serde::Serialize = NoBody> {
    method: Method,
    url: Url,
    body: Option<&'a B>,
    max_attempts: u32,
    json_cap: u64,
}

impl<'a> RemoteRequest<'a, NoBody> {
    /// A retrying GET to `url`.
    pub fn get(url: Url) -> Self {
        Self {
            method: Method::GET,
            url,
            body: None,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            json_cap: DEFAULT_JSON_CAP_BYTES,
        }
    }
}

impl<'a, B: serde::Serialize> RemoteRequest<'a, B> {
    /// A retrying POST to `url` carrying `body` as JSON.
    pub fn post(url: Url, body: &'a B) -> Self {
        Self {
            method: Method::POST,
            url,
            body: Some(body),
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            json_cap: DEFAULT_JSON_CAP_BYTES,
        }
    }

    /// Override the number of attempts (1 = no retry).
    pub fn with_max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Override the response-body size cap for [`remote_request`].
    pub fn with_json_cap(mut self, cap: u64) -> Self {
        self.json_cap = cap;
        self
    }

    /// Execute the request and return the raw response (after retries). The
    /// caller is responsible for inspecting the status.
    pub async fn send_raw(&self) -> Result<reqwest::Response, RemoteAccessError> {
        send_with_retry(&self.method, &self.url, self.max_attempts, || {
            match (&self.method, self.body) {
                (&Method::POST, Some(body)) => {
                    DROP_CLIENT_ASYNC.post(self.url.clone()).json(body)
                }
                (&Method::POST, None) => DROP_CLIENT_ASYNC.post(self.url.clone()),
                _ => DROP_CLIENT_ASYNC.get(self.url.clone()),
            }
        })
        .await
    }
}

/// **The** typed request helper for the `remote` crate. Sends a GET or POST,
/// retries transient failures, and deserializes a 2xx body into `R` with a
/// size cap. Any non-2xx becomes a classified [`RemoteAccessError`]
/// (`Unauthorized` / `ServerError` / `ServerUnavailable`).
///
/// Prefer this over the bare `make_authenticated_*` shims in new code — it
/// gives callers a uniform error taxonomy instead of ad-hoc string errors.
pub async fn remote_request<R, B>(request: RemoteRequest<'_, B>) -> Result<R, RemoteAccessError>
where
    R: DeserializeOwned,
    B: serde::Serialize,
{
    let json_cap = request.json_cap;
    let response = request.send_raw().await?;
    if !response.status().is_success() {
        return Err(classify_error_response(response).await);
    }
    bounded_json(response, json_cap).await
}

/// Like [`remote_request`] but for endpoints whose 2xx response body is
/// uninteresting (the call only needs to know it succeeded). Retries transient
/// failures and maps any non-2xx to a classified [`RemoteAccessError`].
pub async fn remote_request_ok<B>(request: RemoteRequest<'_, B>) -> Result<(), RemoteAccessError>
where
    B: serde::Serialize,
{
    let response = request.send_raw().await?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(classify_error_response(response).await)
    }
}

// ── Backwards-compatible shims ─────────────────────────────────────────
//
// `make_authenticated_get` / `make_authenticated_post` keep their original
// signatures (used by `games`, `download_manager`, `retroarch` and the Tauri
// command layer) but now route through the retry core, so every caller in the
// workspace gets retry + backoff + per-attempt auth for free. New code in this
// crate should prefer `remote_request`.

/// Authenticated, retrying GET. Returns the raw response — callers inspect the
/// status themselves. Errors are the classified [`RemoteAccessError`] variants.
pub async fn make_authenticated_get(url: Url) -> Result<reqwest::Response, RemoteAccessError> {
    RemoteRequest::get(url).send_raw().await
}

/// Authenticated, retrying POST with a JSON body. Returns the raw response.
pub async fn make_authenticated_post<T: serde::Serialize>(
    url: Url,
    body: &T,
) -> Result<reqwest::Response, RemoteAccessError> {
    RemoteRequest::post(url, body).send_raw().await
}

/// The error type the legacy shims previously surfaced. Kept as an alias so
/// the (rare) call sites that named `reqwest_middleware::Error` directly still
/// compile; new code should use [`RemoteAccessError`].
pub type LegacyRequestError = MiddlewareError;

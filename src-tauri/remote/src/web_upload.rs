use base64::Engine;
use database::borrow_db_checked;
use http::{
    HeaderMap, HeaderValue, Method,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use log::warn;

use crate::utils::DROP_CLIENT_ASYNC;

/// Fixed multipart boundary. We control both ends and a collision with the
/// image bytes for this exact ASCII string is astronomically unlikely, so a
/// random boundary isn't worth pulling in reqwest's `multipart` feature.
const UPLOAD_BOUNDARY: &str = "----DropClientUploadBoundary8f2a9c";

/// Upload a user image (avatar / banner) to the backend from native Rust,
/// bypassing the webview's multipart-POST-to-`server://` path.
///
/// That path hard-crashes WebKitGTK on Linux (Steam Deck) — the crash is below
/// the JS layer, so the frontend `try/catch` never sees it. reqwest from Rust
/// behaves identically on every platform, so routing the upload here fixes the
/// Deck without touching the server (it still receives the same
/// `multipart/form-data`). Bytes arrive base64-encoded from the frontend.
///
/// Returns the backend's JSON response body verbatim for the caller to parse.
#[tauri::command]
pub async fn upload_user_image(
    path: String,
    filename: String,
    content_type: String,
    data_base64: String,
) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|e| format!("Invalid image data: {e}"))?;

    // Read auth + base url, then drop the guard before any await (never hold
    // the db lock across a network call — mirrors the server:// proxy).
    let (base_url, web_token) = {
        let db_handle = borrow_db_checked();
        let auth = db_handle
            .auth
            .as_ref()
            .ok_or_else(|| "Not signed in".to_string())?;
        let web_token = auth
            .web_token
            .as_ref()
            .ok_or_else(|| "Not signed in".to_string())?
            .clone();
        (db_handle.base_url.clone(), web_token)
    };

    let url = format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );

    // Strip quotes / CR / LF so a crafted filename or mime can't inject extra
    // headers into the multipart body.
    let sanitise = |s: &str| -> String {
        s.chars()
            .filter(|c| *c != '"' && *c != '\r' && *c != '\n')
            .collect()
    };
    let safe_name = sanitise(&filename);
    let safe_type = {
        let t = sanitise(&content_type);
        if t.is_empty() {
            "application/octet-stream".to_string()
        } else {
            t
        }
    };

    let mut body: Vec<u8> = Vec::with_capacity(bytes.len() + 256);
    body.extend_from_slice(
        format!(
            "--{UPLOAD_BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{safe_name}\"\r\nContent-Type: {safe_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{UPLOAD_BOUNDARY}--\r\n").as_bytes());

    let mut headers = HeaderMap::new();
    let content_type_header = HeaderValue::from_str(&format!(
        "multipart/form-data; boundary={UPLOAD_BOUNDARY}"
    ))
    .map_err(|e| format!("Failed to build content-type header: {e}"))?;
    headers.insert(CONTENT_TYPE, content_type_header);
    let mut auth_header = HeaderValue::from_str(&format!("Bearer {web_token}"))
        .map_err(|e| format!("Failed to build authorization header: {e}"))?;
    auth_header.set_sensitive(true);
    headers.insert(AUTHORIZATION, auth_header);

    let response = DROP_CLIENT_ASYNC
        .request(Method::POST, url.as_str())
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|e| {
            warn!("upload_user_image: request to {url} failed: {e}");
            format!("Upload request failed: {e}")
        })?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        warn!("upload_user_image: backend returned {status}");
        return Err(format!("Upload failed ({status})"));
    }
    Ok(text)
}

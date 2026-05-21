use http::{Response, header::CONTENT_TYPE, response::Builder as ResponseBuilder};
use log::{debug, warn};
use tauri::UriSchemeResponder;

use crate::{
    error::{CacheError, RemoteAccessError},
    requests::{generate_url, make_authenticated_get},
    utils::bounded_bytes,
};

/// Cap for `object://` fetches — game covers, banners, icons. 64 MiB leaves
/// headroom for uncompressed banner art without letting a malicious server
/// drain memory.
const OBJECT_FETCH_CAP: u64 = 64 * 1024 * 1024;

use super::cache::{ObjectCache, cache_object, get_cached_object};

pub async fn fetch_object_wrapper(request: http::Request<Vec<u8>>, responder: UriSchemeResponder) {
    match fetch_object(request).await {
        Ok(r) => responder.respond(r),
        Err(e) => {
            warn!("Cache error: {e}");
            responder.respond(
                Response::builder()
                    .status(500)
                    .body(Vec::new())
                    .expect("Failed to build error response"),
            );
        }
    };
}

pub async fn fetch_object(
    request: http::Request<Vec<u8>>,
) -> Result<Response<Vec<u8>>, CacheError> {
    // Drop leading /
    let object_id = &request.uri().path()[1..];

    let cache_result = get_cached_object::<ObjectCache>(object_id);
    if let Ok(cache_result) = &cache_result
        && !cache_result.has_expired()
    {
        return cache_result.try_into();
    }

    // Route through the shared request helper so object fetches get the same
    // retry/backoff + per-attempt auth as every other Drop API call.
    let url = match generate_url(&["api/v1/client/object", object_id], &[]) {
        Ok(u) => u,
        Err(e) => {
            warn!("Could not build object url for {object_id}: {e}");
            return fallback_to_cache(object_id, cache_result);
        }
    };

    match make_authenticated_get(url).await {
        Ok(r) if r.status().is_success() => {
            // A missing Content-Type used to panic here; default it instead.
            let content_type = r
                .headers()
                .get("Content-Type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();

            match bounded_bytes(r, OBJECT_FETCH_CAP).await {
                Ok(data) => {
                    let resp = ResponseBuilder::new()
                        .header(CONTENT_TYPE, content_type)
                        .body(data)
                        .map_err(CacheError::ConstructionError)?;
                    // Only refresh the cache on a *real* body — never poison it
                    // with an empty payload from a failed read.
                    if cache_result.map_or(true, |x| x.has_expired()) {
                        let to_cache: ObjectCache = resp.clone().try_into()?;
                        if let Err(e) = cache_object::<ObjectCache>(object_id, &to_cache) {
                            warn!("Could not cache object {object_id}: {e}");
                        }
                    }
                    Ok(resp)
                }
                Err(e) => {
                    warn!("Object {object_id} body unreadable ({e}); falling back to cache");
                    fallback_to_cache(object_id, cache_result)
                }
            }
        }
        Ok(r) => {
            warn!(
                "Object fetch for {object_id} returned {}; falling back to cache",
                r.status()
            );
            fallback_to_cache(object_id, cache_result)
        }
        Err(e) => {
            debug!("Object fetch for {object_id} failed ({e}); falling back to cache");
            fallback_to_cache(object_id, cache_result)
        }
    }
}

/// Serve the (possibly stale) cached copy of an object when the network fetch
/// fails. A stale banner beats a broken image.
fn fallback_to_cache(
    object_id: &str,
    cache_result: Result<ObjectCache, RemoteAccessError>,
) -> Result<Response<Vec<u8>>, CacheError> {
    match cache_result {
        Ok(cached) => cached.try_into(),
        Err(e) => {
            warn!("No cached copy of object {object_id}: {e}");
            Err(CacheError::Remote(e))
        }
    }
}

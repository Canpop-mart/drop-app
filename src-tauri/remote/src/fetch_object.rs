use std::{
    collections::HashSet,
    sync::{LazyLock, Mutex},
};

use http::{
    Response, StatusCode,
    header::{CACHE_CONTROL, CONTENT_TYPE},
    response::Builder as ResponseBuilder,
};
use log::{debug, warn};
use tauri::UriSchemeResponder;

use crate::{
    error::CacheError,
    requests::{RemoteRequest, generate_url},
    utils::{OBJECT_FETCH_SEM, OBJECT_REVALIDATE_SEM, bounded_bytes},
};

/// Cap for `object://` fetches — game covers, banners, icons. 64 MiB leaves
/// headroom for uncompressed banner art without letting a malicious server
/// drain memory.
const OBJECT_FETCH_CAP: u64 = 64 * 1024 * 1024;

/// Attempts per object fetch. Deliberately 1 while every other caller in this
/// crate uses 3.
///
/// Over two days of the user's log, retries recovered 5 objects out of 735
/// attempts, while each doomed fetch held a connection for roughly 48 seconds
/// (three 15s timeouts plus backoff) that a live request needed. A stale cached
/// cover is a strictly better answer than a third attempt at a server that is
/// already saturated.
const OBJECT_FETCH_ATTEMPTS: u32 = 1;

use super::cache::{
    ObjectCache, cache_object, forget_object_missing, get_cached_object, note_object_missing,
    object_known_missing,
};

/// Object ids with a background revalidation already running. A grid of two
/// hundred tiles sharing a handful of stale ids would otherwise spawn one
/// refetch per tile.
static REVALIDATING: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Clears the in-flight marker on drop, so a panic mid-refresh cannot wedge an
/// object into "already revalidating" for the rest of the session.
struct RevalidateGuard(String);

impl Drop for RevalidateGuard {
    fn drop(&mut self) {
        if let Ok(mut in_flight) = REVALIDATING.lock() {
            in_flight.remove(&self.0);
        }
    }
}

/// Which pool a fetch competes in. A tile the user is looking at must never
/// queue behind work that only exists to warm a cache.
#[derive(Clone, Copy)]
enum FetchPriority {
    /// A request the webview is blocked on.
    Foreground,
    /// A refresh of a cache entry that has already been served stale.
    Background,
}

impl FetchPriority {
    fn semaphore(self) -> &'static tokio::sync::Semaphore {
        match self {
            Self::Foreground => &OBJECT_FETCH_SEM,
            Self::Background => &OBJECT_REVALIDATE_SEM,
        }
    }
}

/// `Some` if this call won the race to revalidate `object_id`, `None` if
/// another one is already doing it.
fn begin_revalidate(object_id: &str) -> Option<RevalidateGuard> {
    let mut in_flight = REVALIDATING.lock().ok()?;
    if !in_flight.insert(object_id.to_owned()) {
        return None;
    }
    Some(RevalidateGuard(object_id.to_owned()))
}

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

    // Serve ANY cached copy straight away, fresh or stale, and revalidate
    // behind it. Object ids are content-addressed, so a stale body is still the
    // correct body — blocking the paint on a round trip buys nothing, and doing
    // it for every tile at once is what produced the storm.
    if let Ok(cached) = get_cached_object::<ObjectCache>(object_id) {
        let response: Response<Vec<u8>> = (&cached).try_into()?;
        if cached.has_expired() {
            spawn_revalidate(object_id);
        }
        return Ok(response);
    }

    // Nothing cached, and the server recently said this id does not exist.
    if object_known_missing(object_id) {
        debug!("Object {object_id} is known-missing; not re-requesting");
        return not_found();
    }

    refresh_object(object_id, FetchPriority::Foreground).await
}

/// Refresh a stale entry off the request path. The response is discarded; the
/// point is the cache write.
fn spawn_revalidate(object_id: &str) {
    let Some(guard) = begin_revalidate(object_id) else {
        return;
    };
    let object_id = object_id.to_owned();
    tauri::async_runtime::spawn(async move {
        let _guard = guard;
        if let Err(e) = refresh_object(&object_id, FetchPriority::Background).await {
            debug!("Background revalidation of object {object_id} failed: {e}");
        }
    });
}

/// Fetch an object from the server and refresh the cache. Any failure falls
/// back to whatever is already cached.
async fn refresh_object(
    object_id: &str,
    priority: FetchPriority,
) -> Result<Response<Vec<u8>>, CacheError> {
    let url = match generate_url(&["api/v1/client/object", object_id], &[]) {
        Ok(u) => u,
        Err(e) => {
            warn!("Could not build object url for {object_id}: {e}");
            return fallback_to_cache(object_id);
        }
    };

    // Cap concurrent network fetches. Taken *after* the cache lookup above so
    // a cache hit never queues behind the network, and held across the body
    // read so the permit covers the whole socket lifetime. Nothing inside this
    // scope waits on a second permit, so it cannot deadlock, and the RAII guard
    // releases it on panic. Background revalidations queue in their own pool,
    // so hundreds of them cannot get in front of a tile being painted now.
    let _permit = match priority.semaphore().acquire().await {
        Ok(permit) => permit,
        Err(e) => {
            warn!("Object fetch semaphore closed: {e}");
            return fallback_to_cache(object_id);
        }
    };

    // Routed through the shared request helper so object fetches get the same
    // per-attempt auth as every other Drop API call — but with the retry budget
    // cut to one, see OBJECT_FETCH_ATTEMPTS.
    let result = RemoteRequest::get(url)
        .with_max_attempts(OBJECT_FETCH_ATTEMPTS)
        .send_raw()
        .await;

    match result {
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
                        .header(CACHE_CONTROL, crate::cache::OBJECT_CACHE_CONTROL)
                        .body(data)
                        .map_err(CacheError::ConstructionError)?;
                    // Only refresh the cache on a *real* body — never poison it
                    // with an empty payload from a failed read.
                    let to_cache: ObjectCache = resp.clone().try_into()?;
                    if let Err(e) = cache_object::<ObjectCache>(object_id, &to_cache) {
                        warn!("Could not cache object {object_id}: {e}");
                    }
                    forget_object_missing(object_id);
                    Ok(resp)
                }
                Err(e) => {
                    warn!("Object {object_id} body unreadable ({e}); falling back to cache");
                    fallback_to_cache(object_id)
                }
            }
        }
        Ok(r) => {
            let status = r.status();
            if status == StatusCode::NOT_FOUND {
                // A dead id referenced by a hundred rows must cost one request,
                // not a hundred on every render, forever.
                note_object_missing(object_id);
            }
            warn!("Object fetch for {object_id} returned {status}; falling back to cache");
            fallback_to_cache(object_id)
        }
        Err(e) => {
            debug!("Object fetch for {object_id} failed ({e}); falling back to cache");
            fallback_to_cache(object_id)
        }
    }
}

/// Serve the (possibly stale) cached copy of an object when the network fetch
/// fails. A stale banner beats a broken image.
///
/// The entry is re-armed on the way out. Without that, a failed refresh leaves
/// it permanently expired, so every later render sees "stale" and fires the
/// same doomed request — which is exactly the shape in the log, where the
/// hottest object's disk file is frozen at its last success while the same
/// burst repeats across six different days.
fn fallback_to_cache(object_id: &str) -> Result<Response<Vec<u8>>, CacheError> {
    match get_cached_object::<ObjectCache>(object_id) {
        Ok(mut cached) => {
            cached.rearm_after_failure();
            let response: Response<Vec<u8>> = (&cached).try_into()?;
            if let Err(e) = cache_object::<ObjectCache>(object_id, &cached) {
                debug!("Could not re-arm cache entry for object {object_id}: {e}");
            }
            Ok(response)
        }
        Err(e) => {
            warn!("No cached copy of object {object_id}: {e}");
            Err(CacheError::Remote(e))
        }
    }
}

/// An honest 404 for an id we already know the server does not have. An empty
/// body is what an `<img>` needs to give up quietly.
fn not_found() -> Result<Response<Vec<u8>>, CacheError> {
    ResponseBuilder::new()
        .status(StatusCode::NOT_FOUND)
        .body(Vec::new())
        .map_err(CacheError::ConstructionError)
}

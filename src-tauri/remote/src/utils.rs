use std::{
    fs::{self, File},
    io::Read,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use client::{app_state::AppState, app_status::AppStatus};
use database::db::DATA_ROOT_DIR;
use futures_util::StreamExt as _;
use http::Extensions;
use log::{debug, info, warn};
use reqwest::Certificate;
use reqwest_middleware::{
    ClientBuilder, ClientWithMiddleware, Error, Middleware, Next, Result,
    reqwest::{Request, Response},
};
use serde::{Deserialize, de::DeserializeOwned};
use tauri::{AppHandle, Emitter, Manager, async_runtime::Mutex};
use tokio::sync::Semaphore;

use crate::error::RemoteAccessError;

/// How many foreground `object://` fetches may be in flight against the Drop
/// server at once.
///
/// The protocol handlers are registered as an unbounded `spawn` per request, so
/// one library or Community render opened 85 to 200 sockets on the same host
/// through this one shared client. They queued behind each other, all tripped
/// the 15s timeout in the same millisecond, and the JSON the page actually
/// needed starved behind the images (which is why Community rendered blank).
/// Eight is above what a single paint needs to feel instant and below what
/// makes a home NAS fall over.
pub const OBJECT_FETCH_CONCURRENCY: usize = 8;

/// The cap itself. Callers must hold a permit only around the network round
/// trip, never across a wait for a second permit.
pub static OBJECT_FETCH_SEM: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(OBJECT_FETCH_CONCURRENCY));

/// Background cache revalidations get their own, much smaller pool.
///
/// `tokio::sync::Semaphore` is FIFO-fair, so whoever queues first wins. Sharing
/// one pool meant a first render after upgrade (every on-disk entry decodes as
/// stale, so every tile spawns a revalidation) put hundreds of *background*
/// fetches ahead of the next user-visible request. Nothing a user is waiting on
/// may ever queue behind work that exists only to warm a cache, so background
/// work does not get to touch the foreground pools at all.
pub const OBJECT_REVALIDATE_CONCURRENCY: usize = 2;

pub static OBJECT_REVALIDATE_SEM: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(OBJECT_REVALIDATE_CONCURRENCY));

/// `server://` gets a pool of its own, never shared with decorative images.
///
/// Every request through this protocol is a page load or an API call that some
/// surface is blocked on. When it shared the object pool it inherited the whole
/// queue depth of a cold-cache grid: at 8 concurrent, 200 queued images and
/// even half a second each, an iframe or a Community API call waited tens of
/// seconds behind art. A separate pool means image pressure cannot delay it,
/// and the acquire in `server_proto` additionally times out into an unguarded
/// send so a slow burst of `server://` traffic cannot stall a page either.
pub const SERVER_PROTO_CONCURRENCY: usize = 6;

pub static SERVER_PROTO_SEM: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(SERVER_PROTO_CONCURRENCY));

/// Default cap for JSON API responses. Most endpoints return a few kilobytes;
/// 8 MiB leaves headroom for large library/save manifests without letting a
/// malicious server drain memory.
pub const DEFAULT_JSON_CAP_BYTES: u64 = 8 * 1024 * 1024;

/// Deserialize a JSON response body into `T`, rejecting payloads that exceed
/// `cap_bytes`. Enforces the cap both via Content-Length (if provided) and by
/// streaming the body so chunked transfers can't sneak past the limit.
pub async fn bounded_json<T: DeserializeOwned>(
    response: reqwest::Response,
    cap_bytes: u64,
) -> std::result::Result<T, RemoteAccessError> {
    if let Some(len) = response.content_length()
        && len > cap_bytes {
            return Err(RemoteAccessError::ResponseTooLarge(cap_bytes));
        }
    let bytes = bounded_bytes(response, cap_bytes).await?;
    serde_json::from_slice(&bytes)
        .map_err(|e| RemoteAccessError::UnparseableResponse(e.to_string()))
}

/// Collect a response body into a `Vec<u8>`, rejecting payloads that exceed
/// `cap_bytes`. Preferred over `.bytes()` for any server-controlled response
/// where the size isn't bounded by earlier validation.
pub async fn bounded_bytes(
    response: reqwest::Response,
    cap_bytes: u64,
) -> std::result::Result<Vec<u8>, RemoteAccessError> {
    if let Some(len) = response.content_length()
        && len > cap_bytes {
            return Err(RemoteAccessError::ResponseTooLarge(cap_bytes));
        }
    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| RemoteAccessError::FetchErrorLegacy(std::sync::Arc::new(e)))?;
        if buf.len() as u64 + chunk.len() as u64 > cap_bytes {
            return Err(RemoteAccessError::ResponseTooLarge(cap_bytes));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropHealthcheck {
    app_name: String,
}
impl DropHealthcheck {
    pub fn app_name(&self) -> &String {
        &self.app_name
    }
}
static DROP_CERT_BUNDLE: LazyLock<Vec<Certificate>> = LazyLock::new(fetch_certificates);
pub static DROP_CLIENT_SYNC: LazyLock<reqwest::blocking::Client> = LazyLock::new(get_client_sync);
pub static DROP_CLIENT_ASYNC: LazyLock<ClientWithMiddleware> = LazyLock::new(get_client_async);
pub static DROP_CLIENT_DOWNLOAD: LazyLock<ClientWithMiddleware> =
    LazyLock::new(get_client_download);
pub static DROP_CLIENT_WS_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(get_client_ws);

pub static DROP_APP_HANDLE: LazyLock<Mutex<Option<AppHandle>>> = LazyLock::new(|| Mutex::new(None));

/// Lock-free mirror of "the app state currently reads Offline".
///
/// Every successful response used to spawn a task that took the global app
/// handle mutex and then the app state mutex, purely to read a flag that flips
/// at most a handful of times in a session. On a cold library paint that is
/// hundreds of spawns fighting over one lock, and the losers logged
/// `failed to lock app state` — 178 of them in two days of one user's log, none
/// of which meant anything, because there was nothing to change.
///
/// Reading an atomic instead means the recovery path only runs on an actual
/// offline-to-online transition. Anything that puts the app into `Offline` must
/// call [`set_offline_flag`], or the client will stay stuck there.
static IS_OFFLINE: AtomicBool = AtomicBool::new(false);

/// Record that the app state has moved into or out of `Offline`.
pub fn set_offline_flag(offline: bool) {
    IS_OFFLINE.store(offline, Ordering::Relaxed);
}

struct AutoOfflineMiddleware;

#[async_trait::async_trait]
impl Middleware for AutoOfflineMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> Result<Response> {
        let url = req.url().clone();
        let res = next.run(req, extensions).await;
        match res {
            Ok(res) => {
                // The common case — we were already online — costs one atomic
                // load and no task at all.
                if IS_OFFLINE.load(Ordering::Relaxed) {
                    tauri::async_runtime::spawn(async move {
                        let lock = DROP_APP_HANDLE.lock().await;
                        if let Some(app_handle) = &*lock {
                            let state = app_handle.state::<std::sync::nonpoison::Mutex<AppState>>();
                            let state_lock = state.try_lock();
                            if let Ok(mut state_lock) = state_lock {
                                if state_lock.status == AppStatus::Offline {
                                    state_lock.status = AppStatus::SignedIn;
                                    app_handle
                                        .emit("update_state", &*state_lock)
                                        .expect("failed to emit state update");
                                }
                                // Clear only once the state agrees, so a lost
                                // race leaves the next response to retry.
                                set_offline_flag(false);
                            } else {
                                warn!("failed to lock app state - {}", url.as_str());
                            }
                        };
                    });
                }

                Ok(res)
            }
            Err(err) => match err {
                Error::Middleware(error) => Err(Error::Middleware(error)),
                Error::Reqwest(error) => {
                    if error.is_connect() {
                        // Spawn to defer this action - the state will most likely be locked
                        tauri::async_runtime::spawn(async move {
                            let lock = DROP_APP_HANDLE.lock().await;
                            if let Some(app_handle) = &*lock {
                                let state =
                                    app_handle.state::<std::sync::nonpoison::Mutex<AppState>>();
                                let mut state_lock = state.lock();
                                state_lock.status = AppStatus::Offline;
                                // Set under the state mutex, exactly like the
                                // success path clears it. Setting it out here
                                // instead let a concurrent success run its
                                // clear AFTER this spawn's set but BEFORE the
                                // status became Offline, leaving the app
                                // Offline with the flag reading online — which
                                // gates recovery, so nothing could ever heal it
                                // short of a restart.
                                set_offline_flag(true);
                                app_handle
                                    .emit("update_state", &*state_lock)
                                    .expect("failed to emit state update");
                            };
                        });
                    };
                    Err(Error::Reqwest(error))
                }
            },
        }
    }
}

fn fetch_certificates() -> Vec<Certificate> {
    let certificate_dir = DATA_ROOT_DIR.join("certificates");

    let mut certs = Vec::new();
    match fs::read_dir(certificate_dir) {
        Ok(c) => {
            for entry in c {
                match entry {
                    Ok(c) => {
                        let mut buf = Vec::new();
                        match File::open(c.path()) {
                            Ok(f) => f,
                            Err(e) => {
                                warn!(
                                    "Failed to open file at {} with error {}",
                                    c.path().display(),
                                    e
                                );
                                continue;
                            }
                        }
                        .read_to_end(&mut buf)
                        .unwrap_or_else(|e| {
                            warn!(
                                "Failed to read to end of certificate file {} with error {}",
                                c.path().display(),
                                e
                            );
                            0
                        });

                        match Certificate::from_pem_bundle(&buf) {
                            Ok(certificates) => {
                                for cert in certificates {
                                    certs.push(cert);
                                }
                                info!(
                                    "added {} certificate(s) from {}",
                                    certs.len(),
                                    c.file_name().display()
                                );
                            }
                            Err(e) => warn!(
                                "Invalid certificate file {} with error {}",
                                c.path().display(),
                                e
                            ),
                        }
                    }
                    Err(e) => warn!("Skipping certificate directory entry: {e}"),
                }
            }
        }
        Err(e) => {
            debug!("not loading certificates due to error: {e}");
        }
    };
    certs
}

pub fn get_client_sync() -> reqwest::blocking::Client {
    let mut client = reqwest::blocking::ClientBuilder::new();

    for cert in DROP_CERT_BUNDLE.iter() {
        client = client.add_root_certificate(cert.clone());
    }
    client
        .use_rustls_tls()
        .user_agent("Drop Desktop Client")
        .connect_timeout(Duration::from_millis(1500))
        .build()
        .expect("Failed to build synchronous client")
}
pub fn get_client_async() -> ClientWithMiddleware {
    let mut client = reqwest::ClientBuilder::new();

    for cert in DROP_CERT_BUNDLE.iter() {
        client = client.add_root_certificate(cert.clone());
    }
    let normal_client = client
        .use_rustls_tls()
        .user_agent("Drop Desktop Client")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        // This client serves every caller in the workspace, not just the two
        // guarded protocol handlers, so the idle cap is deliberately set above
        // the sum of the guarded pools rather than to any one of them. Sockets a
        // burst opens then get reused instead of being torn down and
        // re-handshaked on the next paint.
        .pool_max_idle_per_host(
            OBJECT_FETCH_CONCURRENCY + OBJECT_REVALIDATE_CONCURRENCY + SERVER_PROTO_CONCURRENCY,
        )
        .build()
        .expect("Failed to build asynchronous client");

    ClientBuilder::new(normal_client)
        .with(AutoOfflineMiddleware)
        .build()
}
/// A dedicated HTTP client for downloading game chunks.
/// Uses a much longer timeout (5 minutes) to handle large chunks on slow connections,
/// while the general-purpose client keeps its 15-second timeout for API calls.
pub fn get_client_download() -> ClientWithMiddleware {
    let mut client = reqwest::ClientBuilder::new();

    for cert in DROP_CERT_BUNDLE.iter() {
        client = client.add_root_certificate(cert.clone());
    }
    let download_client = client
        .use_rustls_tls()
        .user_agent("Drop Desktop Client")
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300)) // 5 minutes for large chunk downloads
        .build()
        .expect("Failed to build download client");

    ClientBuilder::new(download_client)
        .with(AutoOfflineMiddleware)
        .build()
}
pub fn get_client_ws() -> reqwest::Client {
    let mut client = reqwest::ClientBuilder::new();

    for cert in DROP_CERT_BUNDLE.iter() {
        client = client.add_root_certificate(cert.clone());
    }
    client
        .use_rustls_tls()
        .user_agent("Drop Desktop Client")
        .http1_only()
        .build()
        .expect("Failed to build websocket client")
}

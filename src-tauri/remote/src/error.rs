use std::{
    error::Error,
    fmt::{Display, Formatter},
    sync::Arc,
};

use http::{HeaderName, StatusCode, header::ToStrError};
use serde_with::SerializeDisplay;
use url::ParseError;

use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DropServerError {
    pub status_code: usize,
    pub status_message: String,
    pub message: String,
    // pub url: String,
}

/// One coherent taxonomy for everything that can go wrong when talking to the
/// Drop server. The variants below are grouped so callers can branch on the
/// *class* of failure rather than string-matching:
///
/// * **Network / transport** — `FetchError`, `FetchErrorLegacy`, `FetchErrorWS`,
///   `Timeout`. Transient: the shared `remote_request` helper retries these
///   before they ever reach a caller.
/// * **Auth** — `Unauthorized` (401/403 / missing credentials). The user must
///   re-authenticate; retrying will not help.
/// * **Server-side** — `ServerError { status, message }` for any non-2xx the
///   server returned with a parseable body, `ServerUnavailable` for 5xx/429
///   that survived all retries. Distinguishable from a local parse failure.
/// * **Parse** — `UnparseableResponse`, `ParsingError` (URL), `ResponseTooLarge`.
/// * **State / config** — the remainder.
#[derive(Debug, SerializeDisplay)]
pub enum RemoteAccessError {
    FetchErrorLegacy(Arc<reqwest::Error>),
    FetchError(Arc<reqwest_middleware::Error>),
    FetchErrorWS(Arc<reqwest_websocket::Error>),
    /// The request exceeded its deadline after all retries — distinct from a
    /// generic transport error so the UI can say "slow connection".
    Timeout,
    ParsingError(ParseError),
    InvalidEndpoint,
    HandshakeFailed(String),
    GameNotFound(String),
    InvalidResponse(DropServerError),
    /// The server returned a non-2xx status. `status` is the HTTP code so
    /// callers can distinguish auth (401/403), client (4xx) and server (5xx)
    /// failures without re-parsing a string.
    ServerError { status: u16, message: String },
    /// The server is unreachable or returned 5xx/429 on every retry. Retrying
    /// later may succeed; the request itself was well-formed.
    ServerUnavailable(String),
    /// Authentication failed or credentials are missing — the user must
    /// re-authenticate. Retrying with the same credentials will not help.
    Unauthorized,
    UnparseableResponse(String),
    ManifestDownloadFailed(StatusCode, String),
    OutOfSync,
    Cache(std::io::Error),
    CorruptedState,
    NoDepots,
    FailedDownload,
    ResponseTooLarge(u64),
}

impl RemoteAccessError {
    /// True for failures that a retry could plausibly fix — transient
    /// transport errors, timeouts, and server unavailability (5xx/429).
    /// Auth, parse and 4xx errors are *not* retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            RemoteAccessError::Timeout | RemoteAccessError::ServerUnavailable(_) => true,
            RemoteAccessError::FetchError(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            RemoteAccessError::FetchErrorLegacy(e) => {
                e.is_timeout() || e.is_connect() || e.is_request()
            }
            RemoteAccessError::FetchErrorWS(_) => true,
            _ => false,
        }
    }

    /// True when the failure means the user must re-authenticate.
    pub fn is_auth_error(&self) -> bool {
        match self {
            RemoteAccessError::Unauthorized | RemoteAccessError::OutOfSync => true,
            RemoteAccessError::ServerError { status, .. } => *status == 401 || *status == 403,
            RemoteAccessError::InvalidResponse(e) => {
                e.status_code == 401 || e.status_code == 403
            }
            _ => false,
        }
    }
}

impl Display for RemoteAccessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RemoteAccessError::FetchError(error) => {
                if error.is_connect() {
                    return write!(
                        f,
                        "Failed to connect to Drop server. Check if you access Drop through a browser, and then try again."
                    );
                }

                if error.is_timeout() {
                    return write!(
                        f,
                        "Download timed out. This usually means your connection is slow or the server is under heavy load. The download will retry automatically."
                    );
                }

                // Check for body/decoding errors (the common "error decoding response body")
                // These methods are on the inner reqwest::Error
                if let reqwest_middleware::Error::Reqwest(inner) = error.as_ref()
                    && (inner.is_body() || inner.is_decode()) {
                        return write!(
                            f,
                            "Download interrupted — the connection was lost while receiving data. The download will retry automatically."
                        );
                    }

                write!(
                    f,
                    "{}: {}",
                    error,
                    error
                        .source()
                        .map(std::string::ToString::to_string)
                        .unwrap_or("Unknown error".to_string())
                )
            }
            RemoteAccessError::FetchErrorLegacy(error) => {
                if error.is_connect() {
                    return write!(
                        f,
                        "Failed to connect to Drop server. Check if you can access Drop through a browser, and then try again."
                    );
                }

                if error.is_timeout() {
                    return write!(
                        f,
                        "Download timed out. This usually means your connection is slow or the server is under heavy load. The download will retry automatically."
                    );
                }

                if error.is_body() || error.is_decode() {
                    return write!(
                        f,
                        "Download interrupted — the connection was lost while receiving data. The download will retry automatically."
                    );
                }

                write!(
                    f,
                    "{}: {}",
                    error,
                    error
                        .source()
                        .map(|v| v.to_string())
                        .unwrap_or("Unknown error".to_string())
                )
            }
            RemoteAccessError::FetchErrorWS(error) => write!(
                f,
                "{}: {}",
                error,
                error
                    .source()
                    .map(std::string::ToString::to_string)
                    .unwrap_or("Unknown error".to_string())
            ),
            RemoteAccessError::Timeout => write!(
                f,
                "The request to the Drop server timed out. Your connection may be slow or the server under heavy load — please try again."
            ),
            RemoteAccessError::ServerError { status, message } => write!(
                f,
                "The Drop server returned an error ({status}): {message}"
            ),
            RemoteAccessError::ServerUnavailable(detail) => write!(
                f,
                "The Drop server is currently unavailable ({detail}). Please try again in a few moments."
            ),
            RemoteAccessError::Unauthorized => write!(
                f,
                "Your session is no longer valid. Please sign in to Drop again."
            ),
            RemoteAccessError::ParsingError(parse_error) => {
                write!(f, "{parse_error}")
            }
            RemoteAccessError::InvalidEndpoint => write!(f, "invalid drop endpoint"),
            RemoteAccessError::HandshakeFailed(message) => {
                write!(f, "failed to complete handshake: {message}")
            }
            RemoteAccessError::GameNotFound(id) => write!(f, "could not find game on server: {id}"),
            RemoteAccessError::InvalidResponse(error) => write!(
                f,
                "server returned an invalid response: {}, {}",
                error.status_code, error.message
            ),
            RemoteAccessError::UnparseableResponse(error) => {
                write!(f, "server returned an invalid response: {error}")
            }
            RemoteAccessError::ManifestDownloadFailed(status, response) => {
                write!(f, "failed to download game manifest: {status} {response}")
            }
            RemoteAccessError::OutOfSync => write!(
                f,
                "server's and client's time are out of sync. Please ensure they are within at least 30 seconds of each other"
            ),
            RemoteAccessError::Cache(error) => write!(f, "Cache Error: {error}"),
            RemoteAccessError::CorruptedState => write!(
                f,
                "Drop encountered a corrupted internal state. Please report this to the developers, with details of reproduction."
            ),
            RemoteAccessError::NoDepots => write!(
                f,
                "There are no download depots configured on the server. Contact your server admin."
            ),
            RemoteAccessError::FailedDownload => write!(f, "Failed to download."),
            RemoteAccessError::ResponseTooLarge(cap) => write!(
                f,
                "Server response exceeded the maximum allowed size ({cap} bytes)."
            ),
        }
    }
}

impl From<reqwest::Error> for RemoteAccessError {
    fn from(err: reqwest::Error) -> Self {
        RemoteAccessError::FetchErrorLegacy(Arc::new(err))
    }
}
impl From<reqwest_middleware::Error> for RemoteAccessError {
    fn from(value: reqwest_middleware::Error) -> Self {
        RemoteAccessError::FetchError(Arc::new(value))
    }
}
impl From<reqwest_websocket::Error> for RemoteAccessError {
    fn from(err: reqwest_websocket::Error) -> Self {
        RemoteAccessError::FetchErrorWS(Arc::new(err))
    }
}
impl From<ParseError> for RemoteAccessError {
    fn from(err: ParseError) -> Self {
        RemoteAccessError::ParsingError(err)
    }
}
impl std::error::Error for RemoteAccessError {}

#[derive(Debug, SerializeDisplay)]
pub enum CacheError {
    HeaderNotFound(HeaderName),
    ParseError(ToStrError),
    Remote(RemoteAccessError),
    ConstructionError(http::Error),
}

impl Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CacheError::HeaderNotFound(header_name) => {
                format!("Could not find header {header_name} in cache")
            }
            CacheError::ParseError(to_str_error) => {
                format!("Could not parse cache with error {to_str_error}")
            }
            CacheError::Remote(remote_access_error) => {
                format!("Cache got remote access error: {remote_access_error}")
            }
            CacheError::ConstructionError(error) => {
                format!("Could not construct cache body with error {error}")
            }
        };
        write!(f, "{s}")
    }
}

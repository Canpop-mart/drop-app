use std::{
    collections::HashMap,
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use client::{app_status::AppStatus, user::User};
use database::{DatabaseAuth, interface::borrow_db_checked};
use gethostname::gethostname;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use log::{error, warn};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    error::{DropServerError, RemoteAccessError},
    requests::make_authenticated_get,
    utils::DROP_CLIENT_SYNC,
};

use super::{
    cache::{cache_object, get_cached_object},
    requests::generate_url,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityConfiguration {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitiateRequestBody {
    name: String,
    platform: String,
    capabilities: HashMap<String, CapabilityConfiguration>,
    mode: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeRequestBody {
    client_id: String,
    token: String,
}

impl HandshakeRequestBody {
    pub fn new(client_id: String, token: String) -> Self {
        Self { client_id, token }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeResponse {
    private: String,
    certificate: String,
    id: String,
}

impl From<HandshakeResponse> for DatabaseAuth {
    fn from(value: HandshakeResponse) -> Self {
        DatabaseAuth::new(value.private, value.certificate, value.id, None)
    }
}

#[derive(Serialize, Deserialize)]
struct Claims {
    exp: usize,
    nbf: usize,
}

/// Signs a short-lived JWT for the Drop server, as `JWT <client_id> <token>`.
///
/// Returns `Unauthorized` when this device has no usable credentials, rather
/// than panicking. It used to `expect()` on all three failure paths, and a
/// client that had a server URL but no auth — pairing interrupted, or signed
/// out — died on startup before it could draw the sign-in screen, so the user
/// could never recover without deleting the database by hand. There is no
/// request worth making without credentials, so every caller can treat this as
/// "ask the user to sign in again".
pub fn generate_authorization_header() -> Result<String, RemoteAccessError> {
    let certs = {
        let db = borrow_db_checked();
        match db.auth.clone() {
            Some(auth) => auth,
            None => {
                warn!("[AUTH] no credentials on this device; request needs a sign-in");
                return Err(RemoteAccessError::Unauthorized);
            }
        }
    };

    let system_time: usize = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as usize;

    let claims = Claims {
        nbf: system_time,
        exp: system_time + 10,
    };

    // A key we cannot parse or sign with is as unusable as one we never had,
    // and re-pairing is the fix for both, so both map to `Unauthorized`.
    let key = EncodingKey::from_ec_pem(certs.private.as_bytes()).map_err(|e| {
        error!("[AUTH] stored private key is unusable: {e}");
        RemoteAccessError::Unauthorized
    })?;

    let jwt =
        jsonwebtoken::encode(&Header::new(Algorithm::ES384), &claims, &key).map_err(|e| {
            error!("[AUTH] failed to sign request token: {e}");
            RemoteAccessError::Unauthorized
        })?;

    Ok(format!("JWT {} {}", certs.client_id, jwt))
}

pub async fn fetch_user() -> Result<User, RemoteAccessError> {
    let response = make_authenticated_get(generate_url(&["/api/v1/client/user"], &[])?).await?;
    if response.status() != 200 {
        let err: DropServerError = response.json().await?;
        warn!("{err:?}");

        if err.message == "Nonce expired" {
            return Err(RemoteAccessError::OutOfSync);
        }

        return Err(RemoteAccessError::InvalidResponse(err));
    }

    response
        .json::<User>()
        .await
        .map_err(std::convert::Into::into)
}

pub fn auth_initiate_logic(mode: String) -> Result<String, RemoteAccessError> {
    let base_url = {
        let db_lock = borrow_db_checked();
        Url::parse(&db_lock.base_url.clone())?
    };

    let hostname = gethostname();

    let endpoint = base_url.join("/api/v1/client/auth/initiate")?;
    let body = InitiateRequestBody {
        name: format!("{} (Desktop)", hostname.display()),
        platform: env::consts::OS.to_string(),
        capabilities: HashMap::from([
            ("peerAPI".to_owned(), CapabilityConfiguration {}),
            ("trackPlaytime".to_owned(), CapabilityConfiguration {}),
        ]),
        mode,
    };

    let client = DROP_CLIENT_SYNC.clone();
    let response = client.post(endpoint.to_string()).json(&body).send()?;

    if response.status() != 200 {
        let data: DropServerError = response.json()?;
        error!("could not start handshake: {:?}", data);

        return Err(RemoteAccessError::HandshakeFailed(data.message));
    }

    let response = response.text()?;

    Ok(response)
}

pub async fn setup() -> (AppStatus, Option<User>) {
    let auth = {
        let data = borrow_db_checked();
        data.auth.clone()
    };

    if auth.is_some() {
        let user_result = match fetch_user().await {
            Ok(data) => data,
            // Network-class failures (transport error after retries, timeout,
            // or the server being unreachable) mean "go offline and use the
            // cached user" — they are not an auth problem.
            Err(e) if e.is_retryable() => {
                warn!("could not reach server during setup, going offline: {e}");
                let user = get_cached_object::<User>("user").ok();
                // Starting up offline is a transition too: without this the
                // atomic in `utils` would read "online" and no later success
                // would ever move the app back to SignedIn.
                crate::utils::set_offline_flag(true);
                return (AppStatus::Offline, user);
            }
            Err(_) => return (AppStatus::SignedInNeedsReauth, None),
        };
        if let Err(e) = cache_object("user", &user_result) {
            warn!("Could not cache user object with error {e}");
        }
        return (AppStatus::SignedIn, Some(user_result));
    }

    (AppStatus::SignedOut, None)
}

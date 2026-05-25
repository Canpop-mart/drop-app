//! The three Drop-server save-sync endpoints.
//!
//! All three go through the shared [`remote_request`] helper, so they inherit
//! retry/backoff, per-attempt auth and a consistent error taxonomy. The
//! request/response wire structs are private to this module — callers see only
//! the public types re-exported from [`super`].

use std::collections::HashMap;

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::error::RemoteAccessError;
use crate::requests::{generate_url, remote_request, RemoteRequest};

use super::{machine_name, CloudSaveMeta, LocalSaveFile, SyncCheckResponse};

/// Cloud save sync responses can include large binary blobs (base64). Allow
/// up to 512 MiB to cover archives built from many large PC save files.
const SAVE_SYNC_RESPONSE_CAP: u64 = 512 * 1024 * 1024;

// ── Wire types (private) ───────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCheckBody {
    game_id: String,
    local_saves: Vec<SyncCheckLocalEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncCheckLocalEntry {
    filename: String,
    save_type: String,
    data_hash: String,
    client_modified_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkDownloadBody {
    save_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkDownloadResponse {
    saves: Vec<DownloadedSave>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadedSave {
    filename: String,
    save_type: String,
    data_hash: String,
    data: String, // base64
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadBody {
    game_id: String,
    uploaded_from: String,
    saves: Vec<BulkUploadEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadEntry {
    filename: String,
    save_type: String,
    data: String, // base64
    client_modified_at: String,
    data_hash: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadResponse {
    results: Vec<BulkUploadResult>,
    errors: Vec<BulkUploadError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadResult {
    #[allow(dead_code)]
    filename: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadError {
    filename: String,
    error: String,
}

// ── Endpoints ──────────────────────────────────────────────────────────

/// Call the server's sync-check endpoint with local save state.
pub async fn check_sync(
    game_id: &str,
    local_saves: &[LocalSaveFile],
) -> Result<SyncCheckResponse, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/saves/sync-check"], &[])?;
    let body = SyncCheckBody {
        game_id: game_id.to_string(),
        local_saves: local_saves
            .iter()
            .map(|f| SyncCheckLocalEntry {
                filename: f.filename.clone(),
                save_type: f.save_type.clone(),
                data_hash: f.data_hash.clone(),
                client_modified_at: chrono::DateTime::from_timestamp(f.modified_at as i64, 0)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default(),
            })
            .collect(),
    };

    remote_request(RemoteRequest::post(url, &body)).await
}

/// Download cloud saves by their IDs. Returns `(filename, save_type, hash,
/// bytes)` tuples with the base64 payload already decoded.
pub async fn bulk_download(
    save_ids: &[String],
) -> Result<Vec<(String, String, String, Vec<u8>)>, RemoteAccessError> {
    if save_ids.is_empty() {
        return Ok(Vec::new());
    }

    let url = generate_url(&["/api/v1/client/saves/bulk-download"], &[])?;
    let body = BulkDownloadBody {
        save_ids: save_ids.to_vec(),
    };

    let data: BulkDownloadResponse =
        remote_request(RemoteRequest::post(url, &body).with_json_cap(SAVE_SYNC_RESPONSE_CAP))
            .await?;
    let mut results = Vec::new();
    for save in data.saves {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&save.data)
            .map_err(|e| RemoteAccessError::UnparseableResponse(format!("base64 decode: {e}")))?;
        results.push((save.filename, save.save_type, save.data_hash, bytes));
    }
    Ok(results)
}

/// Upload all saves that changed during a session, comparing each file's
/// current hash against the pre-launch snapshot. Returns `(uploaded_count,
/// per-file errors)`.
pub async fn upload_changed_saves(
    game_id: &str,
    pre_launch_hashes: &HashMap<String, String>,
    current_files: &[LocalSaveFile],
) -> Result<(usize, Vec<String>), RemoteAccessError> {
    let mut to_upload = Vec::new();

    for file in current_files {
        let old_hash = pre_launch_hashes.get(&file.filename);
        let changed = match old_hash {
            Some(h) => h != &file.data_hash,
            None => true, // new file
        };
        if !changed {
            continue;
        }

        // Read file data
        let data = match std::fs::read(&file.path) {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    "[SAVE-SYNC] Failed to read {} for upload: {}",
                    file.path.display(),
                    e
                );
                continue;
            }
        };

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&data);

        to_upload.push(BulkUploadEntry {
            filename: file.filename.clone(),
            save_type: file.save_type.clone(),
            data: b64,
            client_modified_at: chrono::DateTime::from_timestamp(file.modified_at as i64, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default(),
            data_hash: file.data_hash.clone(),
        });
    }

    if to_upload.is_empty() {
        info!(
            "[SAVE-SYNC] No saves changed during session for game {}",
            game_id
        );
        return Ok((0, Vec::new()));
    }

    info!(
        "[SAVE-SYNC] Uploading {} changed saves for game {}",
        to_upload.len(),
        game_id
    );

    let url = generate_url(&["/api/v1/client/saves/bulk-upload"], &[])?;
    let body = BulkUploadBody {
        game_id: game_id.to_string(),
        uploaded_from: machine_name(),
        saves: to_upload,
    };

    let data: BulkUploadResponse = remote_request(RemoteRequest::post(url, &body)).await?;
    let uploaded_count = data.results.len();
    let errors: Vec<String> = data
        .errors
        .iter()
        .map(|e| format!("{}: {}", e.filename, e.error))
        .collect();
    for err in &errors {
        warn!("[SAVE-SYNC] Upload error: {}", err);
    }

    Ok((uploaded_count, errors))
}

// ── Per-save endpoints (used by the per-game Cloud Saves panel) ────────
//
// These three are functionally the same shape as the launch-time sync but
// scoped to one save at a time. They live in the same module so the panel
// gets the same JWT/cert auth (via `remote_request`) the launch flow uses
// — the `defineClientEventHandler` server endpoints reject `Bearer
// <web_token>` from the `server://` Tauri protocol, which is why the panel
// can't talk to them directly via `useServerApi()`.

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OneDownloadResponse {
    #[allow(dead_code)]
    filename: String,
    #[allow(dead_code)]
    save_type: String,
    /// base64-encoded payload
    data: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteBody {
    id: String,
    uploaded_from: String,
}

/// List active (non-tombstoned) cloud saves for a game, current user.
pub async fn list_cloud_saves(
    game_id: &str,
) -> Result<Vec<CloudSaveMeta>, RemoteAccessError> {
    let url = generate_url(
        &["/api/v1/client/saves/list"],
        &[("gameId", game_id)],
    )?;
    let saves: Vec<CloudSaveMeta> = remote_request(RemoteRequest::get(url)).await?;
    Ok(saves)
}

/// Download one cloud save by its id. Returns raw decoded bytes.
pub async fn download_cloud_save(id: &str) -> Result<Vec<u8>, RemoteAccessError> {
    let url = generate_url(
        &["/api/v1/client/saves/download"],
        &[("id", id)],
    )?;
    let res: OneDownloadResponse =
        remote_request(RemoteRequest::get(url).with_json_cap(SAVE_SYNC_RESPONSE_CAP)).await?;
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(&res.data)
        .map_err(|e| RemoteAccessError::UnparseableResponse(format!("base64 decode: {e}")))
}

/// Soft-delete one cloud save by id. The server records a tombstone with
/// `deletedFrom = machine_name()` so other devices delete their local
/// copy on next sync.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteResponse {
    #[allow(dead_code)]
    #[serde(default)]
    deleted: bool,
}

pub async fn delete_cloud_save(id: &str) -> Result<(), RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/saves/delete"], &[])?;
    let body = DeleteBody {
        id: id.to_string(),
        uploaded_from: machine_name(),
    };
    let _: DeleteResponse = remote_request(RemoteRequest::post(url, &body)).await?;
    Ok(())
}

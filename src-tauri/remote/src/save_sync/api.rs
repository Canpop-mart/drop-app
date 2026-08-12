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

use super::{CloudSaveGameSummary, CloudSaveMeta, LocalSaveFile, SyncCheckResponse, machine_name};

/// Cloud save sync responses can include large binary blobs (base64). Allow
/// up to 512 MiB to cover archives built from many large PC save files.
const SAVE_SYNC_RESPONSE_CAP: u64 = 512 * 1024 * 1024;

/// Saves per request to `bulk-upload` / `bulk-download`.
///
/// Mirrors `MAX_SAVES_PER_REQUEST` on the server, which rejects the **whole**
/// request with a 400 above this — not the surplus, everything. The recursive
/// emulator scan can turn up thousands of files for a Switch title, so an
/// unchunked call is the normal case there, and the only symptom was one
/// `warn!` in the log while every save silently failed to sync.
const MAX_SAVES_PER_REQUEST: usize = 50;

/// How many requests a batch of `len` saves needs.
fn request_count(len: usize) -> usize {
    len.div_ceil(MAX_SAVES_PER_REQUEST)
}

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

/// Borrows its chunk of `saves` rather than owning it — the entries carry
/// base64 blobs, and cloning them per chunk would double peak memory on
/// exactly the large batches chunking exists for.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadBody<'a> {
    game_id: String,
    uploaded_from: String,
    saves: &'a [BulkUploadEntry],
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

/// One save the server actually stored. `id` is the cloud row's id — without
/// it every manifest entry the client wrote carried `cloudId: null`, so the
/// panel and the tombstone path had no handle on the row they had just
/// created and had to re-derive it from the next sync-check.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BulkUploadResult {
    filename: String,
    id: String,
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
///
/// Sent in chunks of [`MAX_SAVES_PER_REQUEST`] and aggregated, because the
/// server 400s an oversized batch outright.
pub async fn bulk_download(
    save_ids: &[String],
) -> Result<Vec<(String, String, String, Vec<u8>)>, RemoteAccessError> {
    if save_ids.is_empty() {
        return Ok(Vec::new());
    }

    let url = generate_url(&["/api/v1/client/saves/bulk-download"], &[])?;
    if request_count(save_ids.len()) > 1 {
        info!(
            "[SAVE-SYNC] Downloading {} saves in {} requests",
            save_ids.len(),
            request_count(save_ids.len())
        );
    }

    let mut results = Vec::new();
    for chunk in save_ids.chunks(MAX_SAVES_PER_REQUEST) {
        let body = BulkDownloadBody {
            save_ids: chunk.to_vec(),
        };
        let data: BulkDownloadResponse = remote_request(
            RemoteRequest::post(url.clone(), &body).with_json_cap(SAVE_SYNC_RESPONSE_CAP),
        )
        .await?;
        for save in data.saves {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&save.data)
                .map_err(|e| {
                    RemoteAccessError::UnparseableResponse(format!("base64 decode: {e}"))
                })?;
            results.push((save.filename, save.save_type, save.data_hash, bytes));
        }
    }
    Ok(results)
}

/// One file that did not make it into the cloud even though the call as a
/// whole succeeded: the server put it in `errors[]`, or it could not be read
/// off disk in the first place.
///
/// Callers MUST skip these when they write the sync manifest. The manifest
/// records `synced_hash = <current on-disk hash>`, so a file marked synced
/// that was never uploaded hash-matches next session, is never seen as
/// changed, and the user's save silently never reaches the cloud. Chunking
/// makes that the normal failure mode rather than the exception: an oversized
/// batch used to 400 the whole request (a hard `Err`, no manifest write, full
/// retry), whereas chunks return 200 with per-file errors inside.
#[derive(Debug, Clone)]
pub struct UploadFailure {
    pub filename: String,
    pub error: String,
}

impl std::fmt::Display for UploadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.filename, self.error)
    }
}

/// The files an upload against `pre_launch_hashes` would actually send.
///
/// One definition of "changed", shared by the uploader and by anything that
/// needs to reason about an upload before it happens (the quota pre-flight).
/// A file the snapshot has never seen counts as changed, because it is new.
pub fn changed_files<'a>(
    pre_launch_hashes: &HashMap<String, String>,
    current_files: &'a [LocalSaveFile],
) -> Vec<&'a LocalSaveFile> {
    current_files
        .iter()
        .filter(|f| pre_launch_hashes.get(&f.filename) != Some(&f.data_hash))
        .collect()
}

/// Upload all saves that changed during a session, comparing each file's
/// current hash against the pre-launch snapshot.
///
/// Returns `(uploaded, per-file failures)`, where `uploaded` pairs each
/// filename the server stored with its cloud row id. Callers write the
/// manifest from that pair list, never from the input list: a file that only
/// appears in `failures` never reached the cloud, and recording it as synced
/// makes it hash-match forever after so the change is never retried.
pub async fn upload_changed_saves(
    game_id: &str,
    pre_launch_hashes: &HashMap<String, String>,
    current_files: &[LocalSaveFile],
) -> Result<(Vec<(String, String)>, Vec<UploadFailure>), RemoteAccessError> {
    let mut to_upload = Vec::new();
    let mut failures: Vec<UploadFailure> = Vec::new();

    for file in changed_files(pre_launch_hashes, current_files) {
        // Read file data. A file we could not read was not uploaded, so it is
        // a failure like any other — recording it as synced would lose the
        // change just as surely as a server-side error would.
        let data = match std::fs::read(&file.path) {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    "[SAVE-SYNC] Failed to read {} for upload: {}",
                    file.path.display(),
                    e
                );
                failures.push(UploadFailure {
                    filename: file.filename.clone(),
                    error: format!("could not read {}: {e}", file.path.display()),
                });
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
        return Ok((Vec::new(), failures));
    }

    info!(
        "[SAVE-SYNC] Uploading {} changed saves for game {} in {} request(s)",
        to_upload.len(),
        game_id,
        request_count(to_upload.len())
    );

    let url = generate_url(&["/api/v1/client/saves/bulk-upload"], &[])?;
    let device = machine_name();

    // Chunked because the server rejects the entire request above
    // MAX_SAVES_PER_REQUEST. Results and errors are aggregated so the caller
    // sees one combined outcome, exactly as it did for a single request.
    let mut uploaded: Vec<(String, String)> = Vec::new();
    for chunk in to_upload.chunks(MAX_SAVES_PER_REQUEST) {
        let body = BulkUploadBody {
            game_id: game_id.to_string(),
            uploaded_from: device.clone(),
            saves: chunk,
        };
        let data: BulkUploadResponse =
            remote_request(RemoteRequest::post(url.clone(), &body)).await?;
        uploaded.extend(data.results.into_iter().map(|r| (r.filename, r.id)));
        failures.extend(data.errors.into_iter().map(|e| UploadFailure {
            filename: e.filename,
            error: e.error,
        }));
    }
    for err in &failures {
        warn!("[SAVE-SYNC] Upload error: {err}");
    }

    Ok((uploaded, failures))
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

/// One summary row per game the user has cloud saves for.
///
/// The whole-library counterpart to [`list_cloud_saves`]: one request answers
/// "are my saves backed up" for every game at once, where the per-game listing
/// needed one request and one multi-second Ludusavi scan per title.
pub async fn list_cloud_save_summaries() -> Result<Vec<CloudSaveGameSummary>, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/saves/summary"], &[])?;
    remote_request(RemoteRequest::get(url)).await
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
    #[serde(default)]
    deleted: bool,
}

/// Soft-delete the caller's own cloud row for the save `id` names.
///
/// Returns whether anything of the caller's was tombstoned. `false` means the
/// row belongs to another account and the caller has no copy of their own —
/// a delete only ever removes your own copy, so there was nothing to remove.
/// The UI has to say that rather than report a silent success, because the
/// row stays in the listing either way.
pub async fn delete_cloud_save(id: &str) -> Result<bool, RemoteAccessError> {
    let url = generate_url(&["/api/v1/client/saves/delete"], &[])?;
    let body = DeleteBody {
        id: id.to_string(),
        uploaded_from: machine_name(),
    };
    let response: DeleteResponse = remote_request(RemoteRequest::post(url, &body)).await?;
    Ok(response.deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chunk boundaries both bulk endpoints depend on. 51 files used to
    /// mean zero files synced: the server 400s the whole request, so nothing
    /// at all was uploaded, not just the 51st.
    #[test]
    fn batches_split_at_the_server_limit() {
        for (files, expected) in [(0, 0), (1, 1), (49, 1), (50, 1), (51, 2), (100, 2), (101, 3)] {
            assert_eq!(request_count(files), expected, "{files} files");
        }
    }

    #[test]
    fn every_save_lands_in_exactly_one_chunk() {
        for total in [0usize, 1, 49, 50, 51, 137] {
            let ids: Vec<usize> = (0..total).collect();
            let chunks: Vec<&[usize]> = ids.chunks(MAX_SAVES_PER_REQUEST).collect();
            assert_eq!(chunks.len(), request_count(total), "{total} saves");
            assert!(
                chunks.iter().all(|c| c.len() <= MAX_SAVES_PER_REQUEST),
                "a chunk exceeded the server limit at {total} saves"
            );
            let flattened: Vec<usize> = chunks.concat();
            assert_eq!(flattened, ids, "{total} saves");
        }
    }
}

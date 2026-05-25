//! Cloud save synchronisation — automatic pre-launch download and post-exit upload.
//!
//! # Flow
//!
//! **Pre-launch**:
//!   1. Scan local save files (RetroArch `drop-saves` + Ludusavi PC saves) — [`scan`]
//!   2. Compute MD5 of each file — [`scan::md5_file`]
//!   3. POST to `/api/v1/client/saves/sync-check` with local state — [`api::check_sync`]
//!   4. Server compares hashes and returns verdicts: download / upload / conflict / synced
//!   5. If conflicts: emit a Tauri event and **block** until the UI resolves them — [`conflict`]
//!   6. Download cloud saves that are newer — [`api::bulk_download`]
//!   7. Update the local sync manifest — [`manifest`]
//!
//! **Post-exit**:
//!   1. Re-scan local saves, compare MD5 against the pre-launch snapshot
//!   2. Upload any files that changed during the session — [`api::upload_changed_saves`]
//!   3. Update the manifest — non-blocking, runs in background
//!
//! # Module layout
//!
//! This was a single 865-line file; it is now split by concern. Every public
//! item is re-exported from this module, so `remote::save_sync::Foo` paths used
//! by the `process` crate keep working unchanged.
//!
//! * [`manifest`] — the on-disk per-game sync manifest (load / save / repair).
//! * [`scan`]     — discovering local save files (emulator dirs + Ludusavi) and
//!   writing downloaded saves back to disk.
//! * [`api`]      — the three Drop-server save endpoints.
//! * [`conflict`] — turning a sync-check response into UI conflicts and
//!   applying the user's resolutions.
//!
//! This feature is dev-gated; the decomposition does not expand its surface.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod api;
pub mod conflict;
pub mod manifest;
pub mod scan;

// Re-export every public item so existing `remote::save_sync::*` call sites in
// the `process` crate (and elsewhere) keep compiling without edits.
pub use api::{
    bulk_download, check_sync, delete_cloud_save, download_cloud_save, list_cloud_saves,
    upload_changed_saves,
};
pub use conflict::{apply_conflict_resolutions, extract_conflicts, snapshot_hashes};
pub use manifest::{
    load_manifest, manifest_path, save_manifest, update_manifest_after_sync,
};
pub use scan::{
    delete_local_emu_save_for_tombstone, delete_local_pc_save_for_tombstone,
    find_pc_save_destination, md5_file, scan_emu_saves, scan_pc_saves,
    write_downloaded_pc_save, write_downloaded_save,
};

// ── Manifest types (persisted to disk between sessions) ────────────────

/// Per-game sync manifest stored at `{data_dir}/drop/sync-manifests/{game_id}.json`.
/// Tracks which files were last synced and their hashes at sync time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncManifest {
    pub game_id: String,
    pub last_synced_at: Option<String>,
    /// Map of filename → per-file sync state
    pub files: HashMap<String, SyncFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFileEntry {
    pub save_type: String,
    /// MD5 hash of the file at last successful sync
    pub synced_hash: String,
    /// Cloud save ID (for download references)
    pub cloud_id: Option<String>,
    /// Timestamp of last successful sync (ISO 8601)
    pub synced_at: String,
}

// ── Local file snapshot ────────────────────────────────────────────────

/// A snapshot of a local save file — path, hash, and metadata.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSaveFile {
    /// Filename used as the key (e.g. "Game Name.srm" or "pc/save0.dat")
    pub filename: String,
    pub save_type: String,
    /// Full path on disk (needed for reading/writing)
    pub path: PathBuf,
    pub data_hash: String,
    pub size: u64,
    pub modified_at: u64, // unix timestamp
}

// ── Server response types ──────────────────────────────────────────────
//
// Request bodies are private to `api`; these response shapes are public
// because the `process` crate threads them through its sync orchestration.

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncCheckResponse {
    pub actions: Vec<SyncAction>,
    pub cloud_only: Vec<CloudSaveMeta>,
    /// Saves the user deleted from another device. The local copy should be
    /// removed (after a `.bak` backup, same pattern as `write_downloaded_save`).
    /// Defaults to empty when an older server omits the key, so the client
    /// keeps working against pre-T5 servers.
    #[serde(default)]
    pub tombstones: Vec<Tombstone>,
}

/// A cross-device delete record. Surfaces in `SyncCheckResponse.tombstones`
/// when the user soft-deleted a save from another device; this client should
/// delete its local copy.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tombstone {
    pub filename: String,
    /// ISO 8601 timestamp of the soft-delete.
    pub deleted_at: String,
    /// Hostname / friendly device name that initiated the delete.
    /// May be empty.
    #[serde(default)]
    pub deleted_from: String,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAction {
    pub filename: String,
    pub action: String, // "download" | "upload" | "conflict" | "synced"
    pub cloud_save: Option<CloudSaveMeta>,
    pub local_hash: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSaveMeta {
    pub id: String,
    pub filename: String,
    pub save_type: String,
    pub data_hash: String,
    pub size: i64,
    pub uploaded_from: String,
    pub client_modified_at: String,
    pub uploaded_at: String,
}

// ── Event payloads (sent to frontend for conflict UI) ──────────────────

/// Emitted as `save_sync_conflict/{game_id}` when conflicts are detected.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflictEvent {
    pub game_id: String,
    pub conflicts: Vec<SaveConflict>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflict {
    pub filename: String,
    pub save_type: String,
    /// Local file info
    pub local_hash: String,
    pub local_size: u64,
    pub local_modified_at: u64,
    /// Cloud file info
    pub cloud_id: String,
    pub cloud_hash: String,
    pub cloud_size: i64,
    pub cloud_modified_at: String,
    pub cloud_uploaded_from: String,
}

/// The frontend sends this back after the user resolves conflicts.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolution {
    pub filename: String,
    /// "keep_local" or "keep_cloud"
    pub choice: String,
}

// ── Pre-launch sync result ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreLaunchSyncResult {
    pub downloaded: usize,
    pub conflicts_resolved: usize,
    pub pending_uploads: usize,
    pub errors: Vec<String>,
}

// ── Shared helpers ─────────────────────────────────────────────────────

/// Get the device label for `uploadedFrom`. Prefers the user-configured
/// friendly name from settings (e.g. "Marts Desktop", "Steam Deck") and
/// falls back to the raw hostname when it is unset or blank.
pub fn machine_name() -> String {
    if let Some(name) = database::borrow_db_checked().settings.device_name.as_ref() {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    gethostname::gethostname()
        .into_string()
        .unwrap_or_else(|_| "unknown".into())
}

/// Get current time as an ISO 8601 string.
pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

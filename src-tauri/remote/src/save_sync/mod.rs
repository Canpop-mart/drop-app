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
//! The feature is opt-in: every path here is gated on the
//! `cloud_saves_enabled` setting, which defaults to false. Nothing in this
//! module runs for a user who has not turned it on.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod api;
pub mod backup;
pub mod conflict;
pub mod manifest;
pub mod quota;
pub mod scan;
pub mod scope;
pub mod tombstone;

// Re-export every public item so existing `remote::save_sync::*` call sites in
// the `process` crate (and elsewhere) keep compiling without edits.
pub use api::{
    UploadFailure, bulk_download, changed_files, check_sync, delete_cloud_save,
    download_cloud_save, list_cloud_save_summaries, list_cloud_saves, upload_changed_saves,
};
pub use backup::{
    backup_existing, is_backup_artifact, remove_save_file, replace_save_file, write_atomic,
};
pub use conflict::{
    any_conflict_deferred, apply_conflict_resolutions, extract_conflicts, snapshot_hashes,
};
pub use manifest::{
    load_manifest, manifest_path, record_synced_files, save_manifest, update_manifest_after_sync,
};
pub use quota::{
    QuotaPlan, fetch_quota, format_bytes, plan_within_quota, preflight_quota, projected_usage,
    quota_warning,
};
pub use scan::{
    DROP_SAVES_DIR, PcSaveCoverage, SWITCH_SAVE_PREFIX, common_save_root, decode_emu_relpath,
    decode_pc_relpath, decode_switch_relpath, delete_local_emu_save_for_tombstone,
    delete_local_pc_save_for_tombstone, emu_saves_root, encode_pc_filename,
    find_pc_save_destination, is_denylisted_cloud_filename, is_pc_namespaced_filename,
    is_save_denylisted, ludusavi_available, md5_file, pc_save_coverage,
    scan_emu_saves, scan_pc_saves, steam_app_id_for_game, switch_cloud_row_in_scope,
    switch_title_id_from_path, write_downloaded_pc_save, write_downloaded_save,
};
pub use scope::{
    ClaimVerdict, OWNER_CLAIM_FILE, SAVE_SCOPE_MIGRATION_VERSION, USER_ROOT_MARKER, claim_verdict,
    ensure_user_root, is_user_root, read_claim, resolve_emu_saves_root, write_claim,
};
pub use tombstone::{TombstonePlan, plan_tombstones, record_applied};

// ── Manifest types (persisted to disk between sessions) ────────────────

/// Per-game sync manifest stored at
/// `{DATA_ROOT_DIR}/sync-manifests/{user_id}/{game_id}.json`.
/// Tracks which files were last synced and their hashes at sync time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SyncManifest {
    /// The account this manifest belongs to. The directory already says so,
    /// but the server keys every cloud row by user id and this is what lets
    /// [`manifest::load_manifest`] refuse a manifest that ended up in the
    /// wrong tree instead of adopting another person's sync state.
    ///
    /// `#[serde(default)]` because the manifests on disk today predate
    /// per-user scoping; see [`manifest::load_manifest`] for how a blank id
    /// is handled.
    #[serde(default)]
    pub user_id: String,
    pub game_id: String,
    pub last_synced_at: Option<String>,
    /// Map of filename → per-file sync state
    pub files: HashMap<String, SyncFileEntry>,
    /// Tombstones this device has already handled: filename → the
    /// tombstone's `deletedAt`.
    ///
    /// The server re-sends every tombstone on every launch for 30 days. Without
    /// this record the same delete is applied over and over, and the second
    /// pass backs up (then unlinks) the fresh save the game wrote in between.
    /// See [`tombstone`]. Defaults to empty so manifests written before this
    /// field existed still load.
    #[serde(default)]
    pub applied_tombstones: HashMap<String, String>,
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
    /// Display name of the Drop account the row belongs to.
    ///
    /// PC saves are read across every account on the server, because Drop
    /// finds them by where the game writes them on this machine rather than
    /// by who is signed in. When two accounts hold the same filename the
    /// newest one wins, and this is what lets the UI say whose copy that was.
    ///
    /// `#[serde(default)]` so an older server that doesn't send the field
    /// still parses; the UI treats an empty name as "don't show an owner".
    #[serde(default)]
    pub owned_by: String,
    /// The caller's own row for this filename, when another account's row won
    /// the collision and is what `id` points at.
    ///
    /// A losing row used to vanish from every read surface its owner had, so
    /// their own save could not be listed, deleted, or walked back through the
    /// revision history. Present here, it says "you also have a copy of this".
    #[serde(default)]
    pub shadowed_save_id: Option<String>,
    /// Display names of the other accounts holding a save with this filename.
    ///
    /// Non-empty means a second copy exists that this row is hiding. Without
    /// it, a clock running five minutes fast on another machine silently makes
    /// the honest copy invisible with nothing anywhere saying it is there.
    #[serde(default)]
    pub also_held_by: Vec<String>,
}

/// One game's worth of cloud saves, from `/api/v1/client/saves/summary`.
///
/// The library-wide answer to "are my saves backed up". Serialised straight
/// back out to the frontend, so the field names are the wire names.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSaveGameSummary {
    pub game_id: String,
    pub game_name: String,
    pub file_count: u32,
    pub total_bytes: u64,
    /// ISO 8601, server-stamped: when this game's newest save reached the
    /// server. This is the "last backed up" the UI shows, not a client mtime.
    pub last_uploaded_at: String,
    /// ISO 8601 client mtime of the newest save.
    pub last_modified_at: String,
    /// How many of the counted files are another account's copy of a shared
    /// PC save. Non-zero means part of this total is not the caller's own
    /// backup, and a page that hid that would let someone mistake a
    /// housemate's progress for their own safety net.
    #[serde(default)]
    pub shared_count: u32,
    /// How many of the counted files the caller has backed up themselves.
    ///
    /// The endpoint answers "what can you read", and PC saves are readable
    /// across every account on the server, so `file_count` can be entirely a
    /// housemate's work. Zero here means this game is in the list without the
    /// caller having backed up any of it, and nothing may badge it or add it
    /// to a "games backed up" total.
    ///
    /// `None` on a server too old to send it, which is not the same as zero.
    /// The frontend falls back to `file_count - shared_count` there rather
    /// than reporting a library full of nothing.
    #[serde(default)]
    pub own_count: Option<u32>,
    /// Bytes of the counted files that are the caller's own, on the same rule
    /// as `own_count`.
    #[serde(default)]
    pub own_bytes: Option<u64>,
}

/// The signed-in user's storage usage, from `/api/v1/client/saves/quota`.
#[derive(Deserialize, Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSaveQuota {
    pub used_bytes: u64,
    pub limit_bytes: u64,
    /// Storage held by version history. Deliberately NOT counted in
    /// `used_bytes` by the server (see its `fetchUserRevisionBytes`), reported
    /// separately so the figure is visible instead of invisible.
    #[serde(default)]
    pub revision_bytes: u64,
}

// ── Event payloads (sent to frontend for conflict UI) ──────────────────

/// Emitted as `save_sync_conflict` when conflicts are detected.
///
/// One global event, not one topic per game id. A launch can be started from
/// the library page, the Big Picture detail page, or the Big Picture grid's
/// quick-launch, and a per-game topic was only ever heard by a page that
/// happened to be mounted for that exact game. Quick-launch had no listener at
/// all, so a conflict there was invisible and the launch simply stalled.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConflictEvent {
    pub game_id: String,
    pub conflicts: Vec<SaveConflict>,
    /// Seconds the client will wait for an answer before giving up and
    /// syncing nothing, so the dialog's countdown is the real deadline
    /// instead of a number the UI guessed.
    pub timeout_secs: u64,
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
    /// `"keep_local"`, `"keep_cloud"`, or `"skip"` (the dialog was dismissed
    /// without a choice: leave both copies alone).
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
/// friendly name from settings (e.g. "My Desktop", "Steam Deck") and
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

/// The signed-in account's id, or `None` when nobody is signed in.
///
/// Read from the cached `user` object (the same one [`crate::auth::setup`]
/// falls back to) rather than the network, so an offline launch still scopes
/// saves to the right person.
///
/// Deliberately NOT `DatabaseAuth.client_id`: that identifies this *device*.
/// Saves have to follow the person across their machines, not the machine
/// across its people.
pub fn current_user_id() -> Option<String> {
    crate::cache::get_cached_object::<::client::user::User>("user")
        .ok()
        .map(|u| u.id().to_string())
        .filter(|id| !id.is_empty())
}

/// Get current time as an ISO 8601 string.
pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

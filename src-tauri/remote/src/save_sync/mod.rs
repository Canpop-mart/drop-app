//! Cloud save synchronisation — automatic pre-launch download and post-exit upload.
//!
//! # Flow
//!
//! **Pre-launch**:
//!   1. Scan local save files (RetroArch `drop-saves` + Ludusavi PC saves) — [`scan`]
//!   2. Compute MD5 of each file — [`scan::md5_file`]
//!   3. POST to `/api/v1/client/saves/sync-check` with local state — [`api::check_sync`]
//!   4. Server compares hashes and returns verdicts: download / upload / conflict / synced
//!   5. A server "conflict" where only one side changed since the last sync
//!      (per the manifest) becomes a plain download or upload — [`decide`]
//!   6. Remaining conflicts: emit a Tauri event and **block** until the UI
//!      resolves them, or set them aside on a streaming launch — [`conflict`]
//!   7. Download cloud saves that are newer — [`api::bulk_download`]
//!   8. Update the local sync manifest — [`manifest`]
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
//! * [`decide`]   — the three-way verdict (local, cloud, last synced) that
//!   turns a server "conflict" into a plain download or upload when only one
//!   side changed.
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
pub mod decide;
pub mod manifest;
pub mod quota;
pub mod scan;
pub mod scope;
pub mod tombstone;

// Re-export every public item so existing `remote::save_sync::*` call sites in
// the `process` crate (and elsewhere) keep compiling without edits.
pub use api::{
    CloudSaveCurrentVersion, CloudSaveHistory, CloudSaveRestoreResult, CloudSaveRevision,
    UploadFailure, bulk_download, changed_files, check_sync, delete_cloud_save,
    download_cloud_save, list_cloud_save_revisions, list_cloud_save_summaries, list_cloud_saves,
    restore_cloud_save_revision, upload_changed_saves,
};
pub use backup::{
    backup_existing, is_backup_artifact, remove_save_file, replace_save_file, write_atomic,
};
pub use conflict::{
    any_conflict_deferred, apply_conflict_resolutions, extract_conflicts, snapshot_hashes,
};
pub use decide::{SyncVerdict, reclassify_with_manifest, three_way_verdict};
pub use manifest::{
    load_manifest, local_copy_last_synced_by, manifest_path, other_accounts_have_synced,
    record_account_name,
    record_synced_files, save_manifest, update_manifest_after_sync,
};
pub use quota::{
    QuotaPlan, fetch_quota, format_bytes, plan_within_quota, preflight_quota, projected_usage,
    quota_warning,
};
pub use scan::{
    DROP_SAVES_DIR, PcSaveCoverage, PcScanContext, PcScanError, SWITCH_SAVE_PREFIX,
    common_save_root, decode_emu_relpath, pc_scan_context,
    decode_pc_relpath, decode_switch_relpath, delete_local_emu_save_for_tombstone,
    delete_local_pc_save_for_tombstone, emu_saves_root, encode_pc_filename,
    MAX_CLOUD_FILENAME_BYTES, find_pc_save_destination, is_denylisted_cloud_filename,
    is_pc_namespaced_filename, is_shared_between_accounts, legacy_cloud_name,
    scan_emu_saves_all, scan_pc_saves_all, split_too_long_names,
    is_save_denylisted, ludusavi_available, md5_file, pc_save_coverage,
    scan_emu_saves, scan_pc_saves, steam_app_id_for_game, switch_cloud_row_in_scope,
    switch_title_id_from_path, write_downloaded_pc_save, write_downloaded_save,
};
pub use scope::{
    ClaimVerdict, OWNER_CLAIM_FILE, SAVE_SCOPE_MIGRATION_VERSION, USER_ROOT_MARKER, claim_verdict,
    emu_saves_root_is_shared, ensure_user_root, is_user_root, read_claim, resolve_emu_saves_root,
    write_claim,
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
    /// True only on entries written by a build that keeps unanswered
    /// conflicts out of the manifest.
    ///
    /// Older builds recorded a dismissed or timed-out conflict as synced, at
    /// the local hash and against the conflicting cloud row. Read as a
    /// last-sync record, such an entry says "only the cloud changed", and the
    /// three-way check ([`decide::three_way_verdict`]) would download the
    /// cloud copy over the save the user declined to give up. So `synced_hash`
    /// is only used as a base when this is set; every other entry is treated
    /// as "no record" until a successful sync rewrites it.
    ///
    /// `#[serde(default)]` so every manifest already on disk still loads (as
    /// `false`, i.e. untrusted).
    #[serde(default)]
    pub three_way_base: bool,
}

impl SyncFileEntry {
    /// The hash both sides last agreed on, when this entry can be trusted to
    /// say so. See [`SyncFileEntry::three_way_base`].
    pub fn trusted_base(&self) -> Option<&SyncFileEntry> {
        self.three_way_base.then_some(self)
    }
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
    /// Hostname / friendly device name that initiated the delete, for
    /// display. May be empty.
    #[serde(default)]
    pub deleted_from: String,
    /// The Drop client registration (this device's `client_id`) that issued
    /// the delete. What a device recognises its own tombstones by; `None` on
    /// tombstones from before the server recorded it, and from older servers.
    #[serde(default)]
    pub deleted_from_client_id: Option<String>,
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
    /// Saves are read strictly per account now, so this is the signed-in
    /// user's own name. It is still worth carrying: two accounts on one PC
    /// share that PC's save files on disk, and the conflict prompt names whose
    /// cloud copy it is showing.
    ///
    /// `#[serde(default)]` so an older server that doesn't send the field
    /// still parses; the UI treats an empty name as "don't show an owner".
    #[serde(default)]
    pub owned_by: String,
    /// Another account's row shadowing the caller's. Only an older server,
    /// which read PC saves across accounts, ever sends one.
    #[serde(default)]
    pub shadowed_save_id: Option<String>,
    /// Other accounts holding a save with this filename. Only an older
    /// server, which read PC saves across accounts, ever sends any.
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
    /// How many of the counted files are another account's copy. Always 0
    /// from a server that reads saves per account; an older server that read
    /// PC saves across accounts could send more.
    #[serde(default)]
    pub shared_count: u32,
    /// How many of the counted files the caller has backed up themselves.
    /// Equal to `file_count` on a per-account server.
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
    /// Display name of the Drop account the cloud copy belongs to. Empty when
    /// the server did not say.
    pub cloud_owned_by: String,
    /// When the file on disk is exactly what a *different* Drop account on
    /// this PC last synced, that account's display name (or an empty string
    /// when Drop never learned the name). Two accounts on one PC share that
    /// PC's save files, so after switching accounts the local copy can be the
    /// other person's progress, and the prompt has to say so.
    pub local_last_synced_by: Option<String>,
    /// Another Drop account also syncs this game on this device and this file
    /// is one they share (a PC save, the Switch NAND, or an emulator save still
    /// in the old shared folder), so the local copy may
    /// be that person's progress. The prompt then says so rather than "changed
    /// on both sides".
    pub local_may_be_other_account: bool,
    /// Set when the cloud copy is stored under the name an older build of
    /// Drop gave this file: that old name. Keeping the cloud copy writes it
    /// into this file.
    pub cloud_legacy_name: Option<String>,
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

/// This device's Drop client registration id, or `None` when not paired.
///
/// The server stamps it on every tombstone this device creates, and it is what
/// [`tombstone::plan_tombstones`] recognises this device's own deletes by.
pub fn current_client_id() -> Option<String> {
    database::borrow_db_checked()
        .auth
        .as_ref()
        .map(|a| a.client_id.clone())
        .filter(|id| !id.is_empty())
}

/// The signed-in account's display name, for labelling this account's sync
/// state on disk (see [`manifest::record_account_name`]).
pub fn current_user_display_name() -> Option<String> {
    crate::cache::get_cached_object::<::client::user::User>("user")
        .ok()
        .map(|u| u.display_name().to_string())
        .filter(|n| !n.trim().is_empty())
}

/// Get current time as an ISO 8601 string.
pub(crate) fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

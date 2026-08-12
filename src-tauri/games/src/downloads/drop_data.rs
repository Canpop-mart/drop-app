use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

// Monotonic suffix so two concurrent writers (the per-chunk download thread and
// a racing cancel()) never share a temp filename and clobber each other's file.
static DROPDATA_WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

use database::platform::Platform;
use download_manager::error::ApplicationDownloadError;
use log::{error, info};
use utils::lock;

pub type DropData = v1::DropData;

pub static DROPDATA_PATH: &str = ".dropdata";

pub mod v1 {
    use std::{collections::HashMap, path::PathBuf, sync::Mutex};

    use database::platform::Platform;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct DropData {
        pub game_id: String,
        pub game_version: String,
        pub target_platform: Platform,
        // NOTE: no UserConfiguration here. Embedding it made `read()` fail to
        // decode what `write()` produced, so every `.dropdata` looked
        // "corrupt" and the game re-downloaded from scratch. The cause is
        // `UserConfiguration::widescreen`: `AspectRatio` hand-writes its
        // Deserialize on top of `deserialize_any`, and `pot` encodes a unit
        // enum variant as a 1-entry map with no value, so the visitor reads
        // one atom too many and desyncs the rest of the stream. (`serde`
        // defaults are fine in pot; that was the earlier guess.) The field was
        // write-only anyway, the agent carries its own configuration. Files
        // written before the removal are read by `mod v0` below.
        pub contexts: Mutex<HashMap<String, bool>>,
        pub base_path: PathBuf,
        pub previously_installed_version: Option<String>,
    }

    impl DropData {
        pub fn new(
            game_id: String,
            game_version: String,
            target_platform: Platform,
            base_path: PathBuf,
            previously_installed_version: Option<String>,
        ) -> Self {
            Self {
                base_path,
                game_id,
                game_version,
                target_platform,
                contexts: Mutex::new(HashMap::new()),
                previously_installed_version,
            }
        }
    }
}

/// The `.dropdata` layout that shipped before `configuration` was dropped
/// from the struct.
///
/// `pot` writes struct fields by name but its `deserialize_any` cannot skip a
/// unit enum variant (they encode as a 1-entry map with no value, so the
/// "ignore this field" path reads one atom too many and desyncs the rest of
/// the stream). That is why serde's normal unknown-field tolerance does not
/// save us here, and why the current reader fails on every one of these files
/// with `expected identifier, got Map` — see the fixture test below.
///
/// Everything in here is a frozen transcription of the bytes on disk. It must
/// never be re-pointed at the live `database::models` types: those keep
/// evolving (and `AspectRatio` hand-rolls a `deserialize_any` reader that hits
/// the desync described above), which is exactly how this format broke in the
/// first place.
pub mod v0 {
    use std::{collections::HashMap, path::PathBuf, sync::Mutex};

    use database::platform::Platform;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    pub struct DropData {
        pub game_id: String,
        pub game_version: String,
        pub target_platform: Platform,
        pub configuration: LegacyConfiguration,
        pub contexts: Mutex<HashMap<String, bool>>,
        pub base_path: PathBuf,
        pub previously_installed_version: Option<String>,
    }

    /// The old embedded `UserConfiguration`. Read and thrown away: the
    /// download agent is handed its own configuration, so nothing downstream
    /// wants this copy. It only needs to parse so the reader stays in sync
    /// with the rest of the stream.
    ///
    /// Every field defaults, because this struct grew over time and the older
    /// files carry fewer of them.
    #[derive(Deserialize, Debug, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct LegacyConfiguration {
        #[serde(default)]
        pub launch_template: String,
        #[serde(default)]
        pub override_proton_path: Option<String>,
        #[serde(default)]
        pub enable_updates: bool,
        #[serde(default)]
        pub controller_type: Option<LegacyControllerType>,
        #[serde(default)]
        pub quality_preset: Option<LegacyQualityPreset>,
        #[serde(default)]
        pub widescreen: LegacyAspectRatio,
        #[serde(default)]
        pub fullscreen: Option<bool>,
        #[serde(default)]
        pub mangohud: Option<LegacyMangoHudPreset>,
        #[serde(default)]
        pub crt_shader: bool,
    }

    #[derive(Deserialize, Debug)]
    pub enum LegacyControllerType {
        Xbox,
        PlayStation,
        Nintendo,
    }

    #[derive(Deserialize, Debug, Default)]
    pub enum LegacyAspectRatio {
        #[default]
        Standard,
        Wide16_9,
        Wide16_10,
    }

    #[derive(Deserialize, Debug)]
    pub enum LegacyQualityPreset {
        Low,
        Medium,
        High,
        Ultra,
    }

    #[derive(Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub enum LegacyMangoHudPreset {
        Off,
        Minimal,
        Standard,
        Full,
    }

    impl From<DropData> for super::v1::DropData {
        fn from(old: DropData) -> Self {
            Self {
                game_id: old.game_id,
                game_version: old.game_version,
                target_platform: old.target_platform,
                contexts: old.contexts,
                base_path: old.base_path,
                previously_installed_version: old.previously_installed_version,
            }
        }
    }
}

impl DropData {
    /// Load the chunk ledger for an install, or start a new one.
    ///
    /// Fails rather than starting fresh when a `.dropdata` is present but
    /// unreadable. Returning an empty ledger there looks harmless and is not:
    /// every chunk reads as "not downloaded", so a resume, update or repair
    /// silently pulls the entire game again. Ten of these went unnoticed
    /// (including multi-gigabyte Switch titles) because the only symptom was
    /// a warning in the log.
    pub fn generate(
        game_id: String,
        game_version: String,
        target_platform: Platform,
        base_path: PathBuf,
    ) -> Result<Self, ApplicationDownloadError> {
        match DropData::read(&base_path) {
            Ok(v) => {
                if v.game_id != game_id || v.game_version != game_version {
                    // A different game/version occupying this directory is a
                    // legitimate reason to start a new ledger.
                    return Ok(DropData::new(
                        game_id,
                        game_version,
                        target_platform,
                        base_path,
                        Some(v.game_version),
                    ));
                }
                Ok(v)
            }
            Err(e) => {
                let path = base_path.join(DROPDATA_PATH);
                if path.exists() {
                    error!(
                        "refusing to download {game_id}: .dropdata at {} exists but could not \
                         be decoded ({e}). The chunk ledger is intact on disk and is being \
                         left alone. Delete the file to force a full re-download.",
                        path.display()
                    );
                    return Err(ApplicationDownloadError::UnreadableDropData(
                        path.display().to_string(),
                    ));
                }
                // No file at all: a genuinely fresh download.
                Ok(DropData::new(
                    game_id,
                    game_version,
                    target_platform,
                    base_path,
                    None,
                ))
            }
        }
    }
    pub fn read(base_path: &Path) -> Result<Self, io::Error> {
        let mut file = File::open(base_path.join(DROPDATA_PATH))?;

        let mut s = Vec::new();
        file.read_to_end(&mut s)?;

        Self::decode(base_path, &s)
    }

    /// Decode `.dropdata` bytes and point the ledger at the directory it was
    /// actually read from. A file that only the legacy reader understands is
    /// rewritten in the current form so the fallback is paid once per install,
    /// not once per read.
    fn decode(base_path: &Path, bytes: &[u8]) -> Result<Self, io::Error> {
        let (mut data, from_legacy) = Self::decode_any(bytes)?;

        // The ledger is authoritative about chunks, never about where it
        // lives. `base_path` is a copy of wherever the install was when the
        // file was last written, so a library folder that has been moved or
        // renamed carries a path that no longer exists. Everything
        // destructive keys off this field — chunk bytes are written under it,
        // the reconcile sweep hard-unlinks every non-manifest file under it,
        // validation reads it and the install dir recorded in the DB comes
        // from it — so a stale value downloads into and deletes inside the old
        // directory while the DB points at the new one. The directory we just
        // opened the file from is the only trustworthy answer.
        data.base_path = base_path.to_path_buf();

        if from_legacy {
            info!(
                "migrated legacy .dropdata for {} at {}: {} chunk flags preserved",
                data.game_id,
                base_path.display(),
                lock!(data.contexts).len()
            );
            // Written after the fixup so the rewritten file carries the real
            // directory too, and written to that directory rather than to the
            // recorded one.
            data.write_to(base_path);
        }

        Ok(data)
    }

    /// The raw readers: current layout first, then the pre-`configuration`
    /// layout. The `bool` is whether the fallback was the one that worked.
    fn decode_any(bytes: &[u8]) -> Result<(Self, bool), io::Error> {
        let current_err = match pot::from_slice::<v1::DropData>(bytes) {
            Ok(v) => return Ok((v, false)),
            Err(e) => e,
        };

        match pot::from_slice::<v0::DropData>(bytes) {
            Ok(old) => Ok((DropData::from(old), true)),
            Err(legacy_err) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Failed to decode drop data: {current_err} \
                     (legacy reader also failed: {legacy_err})"
                ),
            )),
        }
    }

    pub fn write(&self) {
        self.write_to(&self.base_path);
    }

    fn write_to(&self, dir: &Path) {
        let manifest_raw = match pot::to_vec(&self) {
            Ok(data) => data,
            Err(e) => {
                // Previously this error was swallowed silently. A failure
                // here means the .dropdata marker is not refreshed, which
                // can make a complete install look partial to the next
                // scan — worth a log line.
                error!("failed to serialize .dropdata for {}: {e}", self.game_id);
                return;
            }
        };

        // Atomic write. The old code did `File::create` (which truncates the
        // existing file to zero) then `write_all` — a crash/kill between those
        // two left .dropdata empty or half-written. Because this is written
        // once per completed chunk, that window was hit often, and a corrupt
        // ledger decodes as "no chunks complete" (see `generate`), so resume
        // re-downloaded the ENTIRE game. Write a uniquely-named temp file, then
        // rename it over the real one (rename is atomic on the same
        // filesystem). Mirrors the DB's own atomic write in
        // database/src/interface.rs. No fsync: this guards against a process
        // crash (page cache survives), which is the reported case; per-chunk
        // fsync would throttle the download.
        let final_path = dir.join(DROPDATA_PATH);
        let seq = DROPDATA_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp_path = dir.join(format!("{DROPDATA_PATH}.tmp.{seq}"));

        if let Err(e) = std::fs::write(&tmp_path, &manifest_raw) {
            error!("failed to write temp .dropdata for {}: {e}", self.game_id);
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, &final_path) {
            error!("failed to commit .dropdata for {}: {e}", self.game_id);
            let _ = std::fs::remove_file(&tmp_path);
        }
    }
    pub fn set_contexts(&self, completed_contexts: &[(String, bool)]) {
        *lock!(self.contexts) = completed_contexts
            .iter()
            .map(|s| (s.0.clone(), s.1))
            .collect();
    }
    pub fn set_context(&self, context: String, state: bool) {
        lock!(self.contexts).entry(context).insert_entry(state);
    }
    pub fn get_contexts(&self) -> HashMap<String, bool> {
        lock!(self.contexts).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use database::platform::Platform;
    use serde::Serialize;
    use std::sync::Mutex;

    /// Writer transcribed from a hexdump of a real pre-migration `.dropdata`:
    ///
    /// ```text
    /// Pot\0 game_id .. game_version .. target_platform .. configuration
    ///   { launchTemplate overrideProtonPath enableUpdates controllerType
    ///     qualityPreset widescreen fullscreen mangohud crtShader }
    /// contexts .. base_path .. previously_installed_version
    /// ```
    ///
    /// Kept deliberately separate from `mod v0` so the fallback reader is
    /// tested against the byte layout rather than against itself.
    #[derive(Serialize)]
    enum FixtureAspectRatio {
        Standard,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureConfiguration {
        launch_template: String,
        override_proton_path: Option<String>,
        enable_updates: bool,
        controller_type: Option<()>,
        quality_preset: Option<()>,
        widescreen: FixtureAspectRatio,
        fullscreen: Option<bool>,
        mangohud: Option<()>,
        crt_shader: bool,
    }

    #[derive(Serialize)]
    struct FixtureDropData {
        game_id: String,
        game_version: String,
        target_platform: Platform,
        configuration: FixtureConfiguration,
        contexts: Mutex<HashMap<String, bool>>,
        base_path: PathBuf,
        previously_installed_version: Option<String>,
    }

    fn v0_fixture(base_path: &Path, contexts: &[(&str, bool)]) -> Vec<u8> {
        let fixture = FixtureDropData {
            game_id: "720e0ae9".to_string(),
            game_version: "6c460d6a".to_string(),
            target_platform: Platform::Windows,
            configuration: FixtureConfiguration {
                launch_template: "{}".to_string(),
                override_proton_path: None,
                enable_updates: true,
                controller_type: None,
                quality_preset: None,
                widescreen: FixtureAspectRatio::Standard,
                fullscreen: None,
                mangohud: None,
                crt_shader: false,
            },
            contexts: Mutex::new(
                contexts
                    .iter()
                    .map(|(k, v)| ((*k).to_string(), *v))
                    .collect(),
            ),
            base_path: base_path.to_path_buf(),
            previously_installed_version: None,
        };
        pot::to_vec(&fixture).expect("serialize fixture")
    }

    /// Per-test scratch directory. No tempfile dependency in this crate, and
    /// the tests here only ever touch files they created.
    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("drop_dropdata_test_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// Regression: `.dropdata` that `write()` produced must decode via `read()`.
    #[test]
    fn dropdata_roundtrips_through_pot() {
        let d = DropData::new(
            "g".to_string(),
            "v".to_string(),
            Platform::Windows,
            PathBuf::from("C:/base"),
            None,
        );
        d.set_context("chunk1".to_string(), true);
        let bytes = pot::to_vec(&d).expect("serialize");
        let decoded: DropData = pot::from_slice(&bytes).expect("deserialize");
        assert_eq!(decoded.game_id, "g");
        assert_eq!(decoded.get_contexts().len(), 1);
    }

    /// The fixture really is the shape that broke: the current struct cannot
    /// read it, unknown-field skipping and all.
    #[test]
    fn v0_fixture_defeats_the_current_reader() {
        let bytes = v0_fixture(Path::new("C:/base"), &[("chunk1", true)]);
        assert!(
            bytes
                .windows(b"overrideProtonPath".len())
                .any(|w| w == b"overrideProtonPath"),
            "fixture should carry the removed configuration field"
        );
        assert!(
            pot::from_slice::<v1::DropData>(&bytes).is_err(),
            "if this passes, the fallback reader is no longer exercised"
        );
    }

    #[test]
    fn v0_fixture_decodes_and_converts() {
        let dir = scratch_dir("v0_convert");
        let bytes = v0_fixture(&dir, &[("chunk1", true), ("chunk2", false)]);

        let decoded = DropData::decode(&dir, &bytes).expect("v0 fallback should decode");

        assert_eq!(decoded.game_id, "720e0ae9");
        assert_eq!(decoded.game_version, "6c460d6a");
        assert_eq!(decoded.base_path, dir);
        assert_eq!(decoded.previously_installed_version, None);
        let contexts = decoded.get_contexts();
        assert_eq!(contexts.len(), 2);
        assert_eq!(contexts.get("chunk1"), Some(&true));
        assert_eq!(contexts.get("chunk2"), Some(&false));
    }

    /// The fallback is paid once: reading a v0 file rewrites it in the current
    /// form, so the next read takes the fast path.
    #[test]
    fn reading_a_v0_file_rewrites_it_in_the_current_form() {
        let dir = scratch_dir("v0_rewrite");
        std::fs::write(
            dir.join(DROPDATA_PATH),
            v0_fixture(&dir, &[("chunk1", true)]),
        )
        .expect("seed v0 file");

        DropData::read(&dir).expect("v0 fallback should decode");

        let rewritten = std::fs::read(dir.join(DROPDATA_PATH)).expect("read back");
        assert!(
            !rewritten
                .windows(b"overrideProtonPath".len())
                .any(|w| w == b"overrideProtonPath"),
            "migrated file should no longer carry the removed field"
        );
        let direct: v1::DropData =
            pot::from_slice(&rewritten).expect("migrated file decodes without the fallback");
        assert_eq!(direct.get_contexts().len(), 1);
    }

    /// A ledger written before the library folder moved records the old
    /// directory. Trusting that field means downloading into — and running the
    /// sweep's `remove_file` inside — a directory the DB no longer points at.
    #[test]
    fn a_moved_install_reads_back_pointing_at_the_directory_it_came_from() {
        let dir = scratch_dir("moved_v1");
        let stale = DropData::new(
            "g".to_string(),
            "v".to_string(),
            Platform::Windows,
            PathBuf::from("C:/somewhere/that/moved"),
            None,
        );
        stale.set_context("chunk1".to_string(), true);
        std::fs::write(
            dir.join(DROPDATA_PATH),
            pot::to_vec(&stale).expect("serialize"),
        )
        .expect("seed v1 file");

        let read_back = DropData::read(&dir).expect("v1 file decodes");
        assert_eq!(read_back.base_path, dir);
        assert_eq!(
            read_back.get_contexts().len(),
            1,
            "chunk flags are still trusted"
        );
    }

    /// Same rule for the legacy layout, and the rewritten file must carry the
    /// corrected directory rather than re-persisting the stale one.
    #[test]
    fn a_moved_legacy_install_is_repointed_and_rewritten_with_the_real_directory() {
        let dir = scratch_dir("moved_v0");
        std::fs::write(
            dir.join(DROPDATA_PATH),
            v0_fixture(Path::new("C:/somewhere/that/moved"), &[("chunk1", true)]),
        )
        .expect("seed v0 file");

        let read_back = DropData::read(&dir).expect("v0 fallback should decode");
        assert_eq!(read_back.base_path, dir);

        let rewritten = std::fs::read(dir.join(DROPDATA_PATH)).expect("read back");
        let direct: v1::DropData = pot::from_slice(&rewritten).expect("migrated file decodes");
        assert_eq!(direct.base_path, dir);
    }

    /// The landmine. A `.dropdata` nobody can read must not be answered with a
    /// blank ledger, because a blank ledger means "re-download everything".
    #[test]
    fn unreadable_dropdata_refuses_instead_of_redownloading() {
        let dir = scratch_dir("unreadable");
        let garbage = b"this is not pot data".to_vec();
        std::fs::write(dir.join(DROPDATA_PATH), &garbage).expect("seed garbage");

        let result = DropData::generate(
            "g".to_string(),
            "v".to_string(),
            Platform::Windows,
            dir.clone(),
        );

        match result {
            Err(ApplicationDownloadError::UnreadableDropData(path)) => {
                assert!(path.contains(DROPDATA_PATH));
            }
            Err(other) => panic!("wrong error: {other}"),
            Ok(_) => panic!("generate() handed back a ledger it could not read"),
        }
        assert_eq!(
            std::fs::read(dir.join(DROPDATA_PATH)).expect("file survives"),
            garbage,
            "an unreadable ledger must be left on disk for recovery"
        );
    }

    /// The one case where an empty ledger is correct: nothing downloaded yet.
    #[test]
    fn missing_dropdata_starts_a_fresh_ledger() {
        let dir = scratch_dir("fresh");
        let fresh = DropData::generate(
            "g".to_string(),
            "v".to_string(),
            Platform::Windows,
            dir.clone(),
        )
        .expect("a fresh download has no ledger to lose");
        assert!(fresh.get_contexts().is_empty());
    }
}

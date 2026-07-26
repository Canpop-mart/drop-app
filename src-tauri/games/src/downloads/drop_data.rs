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
use log::error;
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
        // NOTE: no UserConfiguration here. `pot` cannot round-trip a struct with
        // `#[serde(default)]` fields (UserConfiguration has several), so
        // embedding it made `read()` fail to decode what `write()` produced —
        // every `.dropdata` looked "corrupt" and the game re-downloaded from
        // scratch. The field was write-only anyway (the agent carries its own
        // configuration). See the round-trip test below.
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

impl DropData {
    pub fn generate(
        game_id: String,
        game_version: String,
        target_platform: Platform,
        base_path: PathBuf,
    ) -> Self {
        match DropData::read(&base_path) {
            Ok(v) => {
                if v.game_id != game_id || v.game_version != game_version {
                    return DropData::new(
                        game_id,
                        game_version,
                        target_platform,
                        base_path,
                        Some(v.game_version),
                    );
                }
                v
            }
            Err(e) => {
                // Normally this just means "no .dropdata yet" (a fresh
                // download). But if the file EXISTS and still failed to read,
                // it's corrupt — and starting fresh silently re-downloads the
                // whole game, so surface that loudly rather than swallowing it.
                // (Atomic writes above should prevent corruption; this catches
                // the disk-error case.)
                if base_path.join(DROPDATA_PATH).exists() {
                    error!(
                        "corrupt .dropdata for {game_id} ({e}); resume progress lost, \
                         re-downloading from scratch",
                    );
                }
                DropData::new(game_id, game_version, target_platform, base_path, None)
            }
        }
    }
    pub fn read(base_path: &Path) -> Result<Self, io::Error> {
        let mut file = File::open(base_path.join(DROPDATA_PATH))?;

        let mut s = Vec::new();
        file.read_to_end(&mut s)?;

        pot::from_slice(&s).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decode drop data: {e}"),
            )
        })
    }
    pub fn write(&self) {
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
        let final_path = self.base_path.join(DROPDATA_PATH);
        let seq = DROPDATA_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp_path = self
            .base_path
            .join(format!("{DROPDATA_PATH}.tmp.{seq}"));

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

    /// Regression: `.dropdata` that `write()` produced must decode via `read()`.
    /// It didn't while DropData embedded UserConfiguration, because `pot` can't
    /// round-trip a struct with `#[serde(default)]` fields — so every resume saw
    /// a "corrupt" ledger and re-downloaded the whole game.
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
}

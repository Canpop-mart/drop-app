use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

// Monotonic suffix so two concurrent writers never share a temp filename and
// clobber each other's file. Mirrors DROPDATA_WRITE_SEQ in drop_data.rs.
static MODDATA_WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

use database::platform::Platform;
use log::error;
use utils::lock;

pub type ModData = v1::ModData;

/// Directory (relative to the parent game's install dir) that holds one ledger
/// file per installed mod. A mod's files overlay directly into the parent's
/// install dir, so the mod's own resume ledger cannot live at the parent's
/// `.dropdata` (that belongs to the base game). Each mod's ledger instead lives
/// at `<parent install dir>/.mods/<mod game id>.moddata`.
pub static MODS_DIR: &str = ".mods";

/// Build the ledger path for a mod given the parent install dir and the mod's
/// game id.
pub fn moddata_path(base_path: &Path, mod_game_id: &str) -> PathBuf {
    base_path.join(MODS_DIR).join(format!("{mod_game_id}.moddata"))
}

pub mod v1 {
    use std::{collections::HashMap, path::PathBuf, sync::Mutex};

    use database::platform::Platform;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug)]
    pub struct ModData {
        // The mod's OWN game id/version (a mod is a Game with type=Mod).
        pub game_id: String,
        pub game_version: String,
        pub target_platform: Platform,
        // The base game this mod overlays onto.
        pub parent_game_id: String,
        // If set, the executable (relative to the base game's install dir) to
        // launch while this mod is installed, instead of the game's normal one.
        // None for content mods that don't change the launch.
        pub launch_override: Option<String>,
        // NOTE: deliberately NO UserConfiguration here. `pot` cannot round-trip
        // a struct with `#[serde(default)]` fields (UserConfiguration has
        // several), so embedding it makes `read()` fail to decode what `write()`
        // produced. Mods don't need per-mod config anyway — the agent carries
        // its own. (This is the same latent bug that affects DropData resume.)
        // Completed-chunk map (resume ledger), same shape as DropData.
        pub contexts: Mutex<HashMap<String, bool>>,
        // Where the mod's files are written: the PARENT game's install dir.
        pub base_path: PathBuf,
        // Where THIS ledger is serialized: base_path/.mods/<game_id>.moddata.
        pub meta_path: PathBuf,
        // Every file this mod wrote into base_path, POSIX-relative (forward
        // slashes), recorded at completion so uninstall removes exactly these.
        pub installed_files: Mutex<Vec<String>>,
        pub previously_installed_version: Option<String>,
    }

    impl ModData {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            game_id: String,
            game_version: String,
            target_platform: Platform,
            parent_game_id: String,
            launch_override: Option<String>,
            base_path: PathBuf,
            meta_path: PathBuf,
            previously_installed_version: Option<String>,
        ) -> Self {
            Self {
                game_id,
                game_version,
                target_platform,
                parent_game_id,
                launch_override,
                contexts: Mutex::new(HashMap::new()),
                base_path,
                meta_path,
                installed_files: Mutex::new(Vec::new()),
                previously_installed_version,
            }
        }
    }
}

impl ModData {
    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        game_id: String,
        game_version: String,
        target_platform: Platform,
        parent_game_id: String,
        launch_override: Option<String>,
        base_path: PathBuf,
        meta_path: PathBuf,
    ) -> Self {
        match ModData::read(&meta_path) {
            Ok(v) => {
                // A different mod version invalidates the resume ledger — start
                // fresh but remember the old version so the manifest delta can
                // be requested.
                if v.game_id != game_id || v.game_version != game_version {
                    return ModData::new(
                        game_id,
                        game_version,
                        target_platform,
                        parent_game_id,
                        launch_override,
                        base_path,
                        meta_path,
                        Some(v.game_version),
                    );
                }
                v
            }
            Err(e) => {
                // Usually "no ledger yet" (a fresh install). If the file EXISTS
                // and still failed to read it is corrupt; surface that loudly
                // rather than silently re-downloading the whole mod.
                if meta_path.exists() {
                    error!(
                        "corrupt .moddata for {game_id} ({e}); resume progress lost, \
                         re-downloading mod from scratch",
                    );
                }
                ModData::new(
                    game_id,
                    game_version,
                    target_platform,
                    parent_game_id,
                    launch_override,
                    base_path,
                    meta_path,
                    None,
                )
            }
        }
    }

    pub fn read(meta_path: &Path) -> Result<Self, io::Error> {
        let mut file = File::open(meta_path)?;

        let mut s = Vec::new();
        file.read_to_end(&mut s)?;

        pot::from_slice(&s).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decode mod data: {e}"),
            )
        })
    }

    /// Atomic write to meta_path (unique temp + rename), mirroring
    /// DropData::write. The caller must ensure the `.mods` directory exists.
    pub fn write(&self) {
        let manifest_raw = match pot::to_vec(&self) {
            Ok(data) => data,
            Err(e) => {
                error!("failed to serialize .moddata for {}: {e}", self.game_id);
                return;
            }
        };

        let final_path = &self.meta_path;
        let seq = MODDATA_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        let tmp_path = self.meta_path.with_extension(format!("moddata.tmp.{seq}"));

        if let Err(e) = std::fs::write(&tmp_path, &manifest_raw) {
            error!("failed to write temp .moddata for {}: {e}", self.game_id);
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp_path, final_path) {
            error!("failed to commit .moddata for {}: {e}", self.game_id);
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

    pub fn set_installed_files(&self, files: Vec<String>) {
        *lock!(self.installed_files) = files;
    }
    pub fn get_installed_files(&self) -> Vec<String> {
        lock!(self.installed_files).clone()
    }
}

/// Scan a base game's installed mods (`<install_dir>/.mods/*.moddata`) for a
/// launch override and return the first one found. Typically only a loader mod
/// (e.g. SMAPI) sets this, so first-match is sufficient. The launcher calls this
/// to swap the game's executable while such a mod is installed; when the mod is
/// uninstalled its ledger is gone, so this returns None and the game launches
/// normally again.
pub fn find_launch_override(install_dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(install_dir.join(MODS_DIR)).ok()?;
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().ends_with(".moddata") {
            continue;
        }
        if let Ok(m) = ModData::read(&entry.path())
            && let Some(ov) = m.launch_override
            && !ov.trim().is_empty()
        {
            return Some(ov);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ModData {
        let m = ModData::new(
            "gid".to_string(),
            "ver".to_string(),
            Platform::Windows,
            "parent".to_string(),
            Some("Stardew Valley/StardewModdingAPI.exe".to_string()),
            PathBuf::from("C:/base"),
            PathBuf::from("C:/base/.mods/gid.moddata"),
            None,
        );
        m.set_context("chunk1".to_string(), true);
        m.set_installed_files(vec!["a.dll".to_string(), "b/c.dll".to_string()]);
        m
    }

    /// The bug this guards against: a `.moddata` that `write()` produced could
    /// not be decoded by `read()`, so an installed mod always looked
    /// uninstalled. Root cause was embedding UserConfiguration, which `pot`
    /// can't round-trip (it has `#[serde(default)]` fields). Fixed by not
    /// storing config in ModData.
    #[test]
    fn moddata_roundtrips_through_pot() {
        let m = sample();
        let bytes = pot::to_vec(&m).expect("serialize");
        let decoded: ModData = pot::from_slice(&bytes).expect("deserialize");
        assert_eq!(decoded.game_id, "gid");
        assert_eq!(decoded.get_installed_files().len(), 2);
        assert_eq!(decoded.get_contexts().len(), 1);
        assert_eq!(
            decoded.launch_override.as_deref(),
            Some("Stardew Valley/StardewModdingAPI.exe")
        );
    }

}

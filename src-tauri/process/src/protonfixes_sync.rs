//! Sync Drop's bundled umu-protonfixes fixes into the user's localfixes dir.
//!
//! umu-protonfixes reads per-game fix files from
//! `~/.config/protonfixes/localfixes/`, where local files override the
//! built-in ones and a global `default.py` runs for every game. Drop embeds
//! its own `.py` fixes into the binary at compile time and copies them there
//! before a game launches, so umu applies them automatically keyed on the
//! `GAMEID` Drop already passes (the Steam AppID).
//!
//! Ownership is tracked by a first-line SENTINEL marker: we only ever
//! overwrite or remove files carrying it. A user's own localfixes (no marker)
//! are never touched. The whole thing is best-effort and never blocks a
//! launch.

#[cfg(target_os = "linux")]
mod imp {
    use include_dir::{Dir, include_dir};
    use log::{info, warn};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    /// The shipped fixes, embedded at compile time. Packaging-agnostic (the
    /// Flatpak build ships only the bare binary, so Tauri resources wouldn't
    /// reach it).
    static FIXES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/protonfixes");

    /// First-line marker identifying a Drop-managed fix. Present => ours to
    /// overwrite/remove; absent => the user's own file, leave it alone.
    const SENTINEL: &str = "drop-managed";

    static SYNCED: OnceLock<()> = OnceLock::new();

    fn localfixes_dir() -> Option<PathBuf> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("protonfixes").join("localfixes"))
    }

    fn is_drop_managed(path: &Path) -> bool {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| s.lines().next().map(|l| l.contains(SENTINEL)))
            .unwrap_or(false)
    }

    pub fn ensure_synced() {
        // Once per process — the fs work is otherwise repeated every launch.
        if SYNCED.get().is_some() {
            return;
        }

        let Some(dir) = localfixes_dir() else {
            warn!("[protonfixes] no XDG_CONFIG_HOME/HOME set — skipping fix sync");
            return;
        };
        if let Err(e) = fs::create_dir_all(&dir) {
            warn!("[protonfixes] could not create {}: {e}", dir.display());
            return;
        }

        // Names Drop ships this build; used to prune stale Drop files below.
        let mut shipped: Vec<String> = Vec::new();

        for file in FIXES.files() {
            let Some(name) = file.path().file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(".py") {
                continue;
            }
            shipped.push(name.to_string());

            let target = dir.join(name);
            let contents = file.contents();

            if target.exists() {
                if !is_drop_managed(&target) {
                    warn!(
                        "[protonfixes] {name} exists and isn't Drop-managed — leaving the user's file alone"
                    );
                    continue;
                }
                // Ours: rewrite only when the embedded content actually changed.
                if matches!(fs::read(&target), Ok(existing) if existing.as_slice() == contents) {
                    continue;
                }
            }

            match fs::write(&target, contents) {
                Ok(()) => info!("[protonfixes] synced {name}"),
                Err(e) => warn!("[protonfixes] could not write {}: {e}", target.display()),
            }
        }

        // Prune Drop-managed files we no longer ship (never touch user files).
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()).map(String::from) else {
                    continue;
                };
                if name.ends_with(".py") && !shipped.contains(&name) && is_drop_managed(&path) {
                    match fs::remove_file(&path) {
                        Ok(()) => info!("[protonfixes] pruned stale Drop fix {name}"),
                        Err(e) => warn!("[protonfixes] could not prune stale {name}: {e}"),
                    }
                }
            }
        }

        let _ = SYNCED.set(());
    }
}

#[cfg(target_os = "linux")]
pub use imp::ensure_synced;

/// No-op on non-Linux: umu/protonfixes is a Proton-on-Linux concept.
#[cfg(not(target_os = "linux"))]
pub fn ensure_synced() {}

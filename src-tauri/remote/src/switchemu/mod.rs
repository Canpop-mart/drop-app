//! Switch-emulator pre-launch configuration.
//!
//! Structured like [`crate::retroarch`]: [`discovery`] decides what emulator
//! an install directory holds, [`controllers`] turns the connected pad into
//! input config, [`ini`] patches the file without disturbing anything Drop
//! does not own, and [`configure_switch_emu_for_game`] is the launch-time
//! entry point that drives them in order.
//!
//! Two things make this different from the RetroArch path:
//!
//! * **Portable mode is mandatory.** Without a `user/` directory the emulator
//!   writes its config and NAND to `%APPDATA%` on Windows or, on the Deck,
//!   into a Proton prefix Drop throws away — so saves and controller config
//!   both vanish. Drop creates it whether or not it writes any bindings.
//! * **Bindings are reasserted on every launch.** SDL renumbers controller
//!   ports as devices come and go, so a binding that was correct last week can
//!   silently point at nothing. Re-resolving the pad each launch is cheaper
//!   than teaching users to re-map.
//!
//! Everything here is best-effort. Nothing in this module returns an error to
//! the launcher; the worst case is that the emulator starts on its keyboard
//! defaults, which is exactly where it started before this module existed.

pub mod controllers;
pub mod discovery;
pub mod ini;

use log::{info, warn};
use serde::Serialize;
use std::path::PathBuf;

pub use controllers::{GuidSource, PadIdentity};
pub use discovery::{SwitchEmuFamily, detect_switch_emulator};

/// The `qt-config.ini` section player settings live in.
///
/// `Settings::TranslateCategory(Category::Controls)` returns the literal
/// `"Controls"` (eden `src/common/settings.cpp:284-285`), and only one group
/// is pushed, so keys are written flat inside it with no prefix
/// (`src/frontend_common/config.cpp:964-1029`).
const CONTROLS_SECTION: &str = "Controls";

/// What [`configure_switch_emu_for_game`] did, for logs and the launch trace.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SwitchEmuOutcome {
    /// No Switch emulator in this directory — the common case, since this runs
    /// on every emulator launch.
    NotSwitchEmulator,
    /// Ryujinx: detected, deliberately untouched.
    UnsupportedEmulator { emulator: String },
    /// Portable mode is set up but no controller was connected, so the user's
    /// keyboard defaults were left alone.
    NoPadConnected { emulator: String, portable_dir: String },
    /// A pad is connected but SDL's database has no unambiguous entry for it,
    /// so Drop has no raw button indices it can trust.
    NoKnownMapping {
        emulator: String,
        guid: String,
        pad_name: String,
    },
    /// Bindings written.
    Configured {
        emulator: String,
        config_path: String,
        guid: String,
        guid_source: GuidSource,
        port: u32,
        pad_name: String,
        mapping_name: String,
        keys_written: usize,
    },
    /// Something on disk refused to cooperate. Never fatal to a launch.
    Failed { emulator: String, error: String },
}

/// Detects the Switch emulator at `emulator_install_dir`, forces portable
/// mode, and rewrites the player-1 input keys for `pad`.
///
/// Pass `None` for `pad` when no controller is connected: portable mode is
/// still set up, but no binding is written, so a keyboard player keeps their
/// working setup.
///
/// The emulator rewrites the whole config at startup *and* at exit
/// (`src/frontend_common/config.cpp:873-877`), so this must run before the
/// process spawns — anything written while it is running is lost.
pub fn configure_switch_emu_for_game(
    emulator_install_dir: &str,
    pad: Option<&PadIdentity>,
) -> SwitchEmuOutcome {
    let emu_root = PathBuf::from(emulator_install_dir);

    let family = match detect_switch_emulator(&emu_root) {
        Some(f) => f,
        None => return SwitchEmuOutcome::NotSwitchEmulator,
    };
    let emulator = family.label().to_string();

    if matches!(family, SwitchEmuFamily::Ryujinx { .. }) {
        warn!(
            "[SWITCHEMU] Ryujinx detected in {emulator_install_dir} — Drop has no Ryujinx input \
             writer, leaving its config alone"
        );
        return SwitchEmuOutcome::UnsupportedEmulator { emulator };
    }

    let portable_dir = match discovery::ensure_portable_dir(&emu_root) {
        Ok(d) => d,
        Err(e) => {
            warn!("[SWITCHEMU] Could not create portable dir in {emulator_install_dir}: {e}");
            return SwitchEmuOutcome::Failed {
                emulator,
                error: format!("portable dir: {e}"),
            };
        }
    };
    info!(
        "[SWITCHEMU] {emulator} portable root: {} (Linux needs the child cwd set here)",
        portable_dir.display()
    );

    let Some(pad) = pad else {
        info!("[SWITCHEMU] No controller connected — keeping the emulator's keyboard defaults");
        return SwitchEmuOutcome::NoPadConnected {
            emulator,
            portable_dir: portable_dir.display().to_string(),
        };
    };

    let Some(mapping) = controllers::lookup_pad_mapping(&pad.guid) else {
        warn!(
            "[SWITCHEMU] '{}' (guid {}) is not in SDL's controller database — writing no bindings \
             rather than guessing raw button indices",
            pad.name, pad.guid
        );
        return SwitchEmuOutcome::NoKnownMapping {
            emulator,
            guid: pad.guid.clone(),
            pad_name: pad.name.clone(),
        };
    };

    let entries = controllers::build_player0_entries(pad, &mapping);
    if entries.is_empty() {
        return SwitchEmuOutcome::NoKnownMapping {
            emulator,
            guid: pad.guid.clone(),
            pad_name: pad.name.clone(),
        };
    }

    let config_path = discovery::qt_config_path(&emu_root);
    // One-shot backup so a user who had mapped their pad by hand can get it
    // back. Drop only rewrites the keys it owns, but those are precisely the
    // keys a hand-mapped config uses.
    back_up_config_once(&config_path);

    if let Err(e) = ini::patch_ini_section(&config_path, CONTROLS_SECTION, &entries) {
        warn!("[SWITCHEMU] Failed to write {}: {e}", config_path.display());
        return SwitchEmuOutcome::Failed {
            emulator,
            error: format!("{}: {e}", config_path.display()),
        };
    }

    controllers::log_binding_summary(pad, &mapping, &entries);
    SwitchEmuOutcome::Configured {
        emulator,
        config_path: config_path.display().to_string(),
        guid: pad.guid.clone(),
        guid_source: pad.guid_source,
        port: pad.port,
        pad_name: pad.name.clone(),
        mapping_name: mapping.display_name.clone(),
        keys_written: entries.len(),
    }
}

/// Copies `qt-config.ini` to `qt-config.ini.drop-backup` the first time Drop
/// touches it. Later launches leave the backup alone, so it always holds the
/// config as it was before Drop ever wrote input keys.
fn back_up_config_once(config_path: &std::path::Path) {
    if !config_path.is_file() {
        return;
    }
    let backup = config_path.with_extension("ini.drop-backup");
    if backup.exists() {
        return;
    }
    match std::fs::copy(config_path, &backup) {
        Ok(_) => info!("[SWITCHEMU] Backed up existing config to {}", backup.display()),
        Err(e) => warn!("[SWITCHEMU] Could not back up {}: {e}", config_path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "drop-switchemu-mod-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn xbox_pad() -> PadIdentity {
        PadIdentity {
            guid: "030000005e0400008e02000010010000".into(),
            port: 0,
            name: "Xbox 360 Controller".into(),
            guid_source: GuidSource::Observed,
        }
    }

    #[test]
    fn plain_directory_is_not_a_switch_emulator() {
        let dir = tmpdir("plain");
        assert!(matches!(
            configure_switch_emu_for_game(&dir.display().to_string(), None),
            SwitchEmuOutcome::NotSwitchEmulator
        ));
    }

    #[test]
    fn ryujinx_is_detected_and_skipped() {
        let dir = tmpdir("ryujinx");
        std::fs::write(dir.join("Ryujinx.exe"), b"").unwrap();
        let outcome = configure_switch_emu_for_game(&dir.display().to_string(), Some(&xbox_pad()));
        assert!(
            matches!(outcome, SwitchEmuOutcome::UnsupportedEmulator { .. }),
            "{outcome:?}"
        );
        // Nothing written — not even the portable dir.
        assert!(!dir.join("user").exists());
    }

    #[test]
    fn no_pad_writes_no_bindings_but_sets_up_portable_mode() {
        let dir = tmpdir("nopad");
        std::fs::write(dir.join("eden.exe"), b"").unwrap();
        let outcome = configure_switch_emu_for_game(&dir.display().to_string(), None);
        assert!(
            matches!(outcome, SwitchEmuOutcome::NoPadConnected { .. }),
            "{outcome:?}"
        );
        assert!(dir.join("user").is_dir());
        assert!(!discovery::qt_config_path(&dir).exists());
    }

    #[test]
    fn configures_and_rewrites_idempotently_without_disturbing_other_keys() {
        let dir = tmpdir("configure");
        std::fs::write(dir.join("eden.exe"), b"").unwrap();
        let config = discovery::qt_config_path(&dir);
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(
            &config,
            "[Controls]\nplayer_0_button_a=engine:keyboard,code:67,toggle:0\n\
             player_0_button_a\\default=true\nvibration_enabled=true\n\n\
             [Renderer]\nbackend=1\n",
        )
        .unwrap();

        let outcome = configure_switch_emu_for_game(&dir.display().to_string(), Some(&xbox_pad()));
        let SwitchEmuOutcome::Configured { keys_written, .. } = &outcome else {
            panic!("expected Configured, got {outcome:?}");
        };
        assert!(*keys_written > 0);

        let first = std::fs::read_to_string(&config).unwrap();
        assert!(first.contains("player_0_button_a=engine:sdl,"));
        // The silent-failure guard: the stale default twin must be flipped.
        assert!(first.contains("player_0_button_a\\default=false"));
        assert!(!first.contains("engine:keyboard"));
        // Keys Drop does not own survive, in their own sections.
        assert!(first.contains("vibration_enabled=true"));
        assert!(first.contains("[Renderer]\nbackend=1"));
        // And the pre-Drop config is recoverable.
        assert!(config.with_extension("ini.drop-backup").is_file());

        configure_switch_emu_for_game(&dir.display().to_string(), Some(&xbox_pad()));
        let second = std::fs::read_to_string(&config).unwrap();
        assert_eq!(first, second, "second run changed the file");
    }
}

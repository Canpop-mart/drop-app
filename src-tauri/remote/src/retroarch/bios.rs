//! BIOS / firmware detection and auto-placement.
//!
//! Many libretro cores need BIOS files in a specific subdirectory under
//! `system/`. Users routinely drop them straight into `system/`, so Drop
//! detects them there and auto-copies them to where the core looks. Any
//! genuinely-missing BIOS produces a warning the frontend can surface.
//!
//! The [`BIOS_SPECS`] table is data-driven — one row per system.

use log::{info, warn};
use std::fs;
use std::path::Path;

/// One BIOS-detection rule.
struct BiosSpec {
    /// Human-readable system label (shown in warnings).
    label: &'static str,
    /// ROM extensions that use this system's BIOS. The check is skipped
    /// unless the launching game's extension is in this list.
    rom_extensions: &'static [&'static str],
    /// Subdirectory under `system/` where the core looks. `None` = `system/`
    /// root. When set, BIOS files found in the root are auto-copied here.
    core_subdir: Option<&'static str>,
    /// Returns `true` if a (lowercased) filename is a BIOS for this system.
    matches: fn(&str) -> bool,
    /// Example filename shown in the warning message.
    example: &'static str,
    /// `true` if a missing BIOS makes the game crash on launch (vs merely
    /// risking a black screen).
    crash_on_missing: bool,
}

fn is_ps1_bios(name: &str) -> bool {
    name == "scph5501.bin" || name == "scph1001.bin" || name == "psxonpsp660.bin"
}
fn is_ps2_bios(name: &str) -> bool {
    (name.starts_with("scph") || name.starts_with("ps2"))
        && name.ends_with(".bin")
        && !name.contains("scph5501")
        && !name.contains("scph1001")
}
fn is_nds_bios(name: &str) -> bool {
    name == "bios7.bin" || name == "bios9.bin" || name == "firmware.bin"
}
fn is_segacd_bios(name: &str) -> bool {
    (name.starts_with("bios_cd_") && name.ends_with(".bin")) || name == "bios_md.bin"
}
fn is_saturn_bios(name: &str) -> bool {
    name == "sega_101.bin" || name == "mpr-17933.bin" || name == "saturn_bios.bin"
}
fn is_gba_bios(name: &str) -> bool {
    name == "gba_bios.bin"
}

const BIOS_SPECS: &[BiosSpec] = &[
    BiosSpec {
        label: "PS1",
        rom_extensions: &["cue", "bin", "chd", "pbp"],
        core_subdir: None,
        matches: is_ps1_bios,
        example: "scph5501.bin or scph1001.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "PS2",
        rom_extensions: &["iso", "chd", "cue", "bin"],
        core_subdir: Some("pcsx2/bios"),
        matches: is_ps2_bios,
        example: "SCPH-70012.bin",
        crash_on_missing: true,
    },
    BiosSpec {
        label: "NDS",
        rom_extensions: &["nds"],
        core_subdir: None,
        matches: is_nds_bios,
        example: "bios7.bin, bios9.bin, firmware.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "Sega CD",
        rom_extensions: &["chd"],
        core_subdir: None,
        matches: is_segacd_bios,
        example: "bios_cd_u.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "Saturn",
        rom_extensions: &["chd"],
        core_subdir: None,
        matches: is_saturn_bios,
        example: "sega_101.bin or mpr-17933.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "GBA",
        rom_extensions: &["gba"],
        core_subdir: None,
        matches: is_gba_bios,
        example: "gba_bios.bin",
        crash_on_missing: false,
    },
];

/// Detects BIOS files relevant to the launching ROM, auto-copies any found in
/// `system/` into the subdirectory the core expects, and returns one warning
/// string per system whose BIOS is still missing.
///
/// `current_rom_ext` is the lowercased extension of the launching ROM (or
/// `None` if unknown — then all checks run).
pub fn check_and_place_bios(system_dir: &Path, current_rom_ext: Option<&str>) -> Vec<String> {
    let mut warnings = Vec::new();

    for spec in BIOS_SPECS {
        // Skip BIOS checks irrelevant to the current game.
        if !spec.rom_extensions.is_empty()
            && let Some(ext) = current_rom_ext
                && !spec.rom_extensions.contains(&ext) {
                    continue;
                }

        let target_dir = match spec.core_subdir {
            Some(sub) => system_dir.join(sub),
            None => system_dir.to_path_buf(),
        };

        if spec.core_subdir.is_some()
            && let Err(e) = fs::create_dir_all(&target_dir) {
                warn!(
                    "[RETROARCH] Failed to create {} BIOS dir {}: {e}",
                    spec.label,
                    target_dir.display()
                );
            }

        let has_bios_in_target = dir_has_bios(&target_dir, spec.matches);

        // Core expects a subdir — auto-copy any BIOS sitting in system/ root.
        if !has_bios_in_target && spec.core_subdir.is_some() {
            let root_bios: Vec<_> = fs::read_dir(system_dir)
                .into_iter()
                .flat_map(|entries| entries.filter_map(Result::ok))
                .filter(|e| (spec.matches)(&e.file_name().to_string_lossy().to_lowercase()))
                .collect();

            if !root_bios.is_empty() {
                info!(
                    "[RETROARCH] Found {} {} BIOS file(s) in system/ — copying to {}",
                    root_bios.len(),
                    spec.label,
                    target_dir.display()
                );
                for entry in &root_bios {
                    let dest = target_dir.join(entry.file_name());
                    if !dest.exists() {
                        match fs::copy(entry.path(), &dest) {
                            Ok(_) => info!(
                                "[RETROARCH] Copied BIOS: {} -> {}",
                                entry.path().display(),
                                dest.display()
                            ),
                            Err(e) => warn!(
                                "[RETROARCH] Failed to copy BIOS {} -> {}: {e}",
                                entry.path().display(),
                                dest.display()
                            ),
                        }
                    }
                }
            }
        }

        // Re-check after the potential copy.
        if dir_has_bios(&target_dir, spec.matches) {
            info!("[RETROARCH] {} BIOS found in {}", spec.label, target_dir.display());
        } else {
            let action = if spec.crash_on_missing {
                "will crash on launch"
            } else {
                "may not boot correctly"
            };
            let msg = format!(
                "No {} BIOS found. {} games {action}. Place {} in {}",
                spec.label,
                spec.label,
                spec.example,
                target_dir.display()
            );
            warn!("[RETROARCH] {msg}");
            warnings.push(msg);
        }
    }

    warnings
}

/// Returns `true` if `dir` contains a file matching `matches`.
fn dir_has_bios(dir: &Path, matches: fn(&str) -> bool) -> bool {
    fs::read_dir(dir)
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .any(|e| matches(&e.file_name().to_string_lossy().to_lowercase()))
}

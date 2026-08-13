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
use std::path::{Path, PathBuf};

/// One BIOS-detection rule.
struct BiosSpec {
    /// Human-readable system label (shown in warnings).
    label: &'static str,
    /// ROM extensions that use this system's BIOS. The check is skipped
    /// unless the launching game's extension is in this list.
    rom_extensions: &'static [&'static str],
    /// libretro core name fragments that belong to this system, matched against
    /// the filename of the core the ROM actually resolved to. Extensions are
    /// shared across disc systems, so this is what keeps a PS1 `.cue` from
    /// reporting a missing PS2, Sega CD and Saturn BIOS.
    cores: &'static [&'static str],
    /// Subdirectories under `system/` where the core looks, relative to
    /// `system/`. Empty = the `system/` root, which is what most cores use.
    /// When non-empty, BIOS files found in the root are auto-copied into each
    /// of them.
    core_subdirs: &'static [&'static str],
    /// Returns `true` if a (lowercased) filename belongs to this system's
    /// BIOS set. Everything matching is placed for the core.
    matches: fn(&str) -> bool,
    /// Returns `true` if a (lowercased) filename is on its own enough to boot.
    /// `None` = every [`BiosSpec::matches`] hit counts, which is right for the
    /// systems whose BIOS is a single file. PS2 dumps are a set — the
    /// `.EROM`/`.NVM`/`.ROM1`/`.ROM2` companions must travel with the image
    /// but PCSX2 cannot boot from them alone, so they must not satisfy the
    /// "BIOS present" check on their own.
    satisfies: Option<fn(&str) -> bool>,
    /// Example filename shown in the warning message.
    example: &'static str,
    /// `true` if a missing BIOS makes the game crash on launch (vs merely
    /// risking a black screen).
    crash_on_missing: bool,
}

/// PS1 BIOS images. They share the `scph` prefix with the PS2 set, so the PS2
/// matcher has to exclude them by name: without this `scph5500.bin` was copied
/// into `pcsx2/bios/` and counted as a PS2 BIOS, which silenced the "no PS2
/// BIOS" warning while PCSX2 still refused to boot.
const PS1_BIOS_NAMES: &[&str] = &[
    "scph1000.bin",
    "scph1001.bin",
    "scph1002.bin",
    "scph101.bin",
    "scph102.bin",
    "scph5000.bin",
    "scph5500.bin",
    "scph5501.bin",
    "scph5502.bin",
    "scph5552.bin",
    "scph7001.bin",
    "scph7002.bin",
    "scph7003.bin",
    "scph7502.bin",
    "psxonpsp660.bin",
];

/// File extensions in a PCSX2 BIOS dump. The image is `.bin`; the rest are the
/// NVRAM and expansion ROM regions PCSX2 reads from alongside it.
const PS2_BIOS_EXTS: &[&str] = &["bin", "erom", "nvm", "rom1", "rom2", "mec"];

fn is_ps1_bios(name: &str) -> bool {
    name == "scph5501.bin" || name == "scph1001.bin" || name == "psxonpsp660.bin"
}
fn is_ps2_bios(name: &str) -> bool {
    if PS1_BIOS_NAMES.contains(&name) {
        return false;
    }
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    // A PCSX2 dump is named after the console it came from: `SCPH-70004_BIOS_
    // V12_PAL_200.BIN` and friends. Matching on the extension set (not just
    // `.bin`) is what makes the companion files travel with the image — the
    // old `.bin`-only rule left `.EROM`/`.NVM`/`.ROM1`/`.ROM2` behind in
    // `system/`, where PCSX2 never looks for them.
    PS2_BIOS_EXTS.contains(&ext) && (stem.starts_with("scph") || stem.starts_with("ps2"))
}
/// The bootable half of a PS2 dump: the ROM image itself.
fn is_ps2_bios_image(name: &str) -> bool {
    is_ps2_bios(name) && name.ends_with(".bin")
}
fn is_nds_bios(name: &str) -> bool {
    name == "bios7.bin" || name == "bios9.bin" || name == "firmware.bin"
}
fn is_dreamcast_bios(name: &str) -> bool {
    name == "dc_boot.bin" || name == "dc_flash.bin" || name == "boot.bin"
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
        cores: &["beetle_psx", "mednafen_psx", "pcsx_rearmed", "swanstation", "duckstation"],
        core_subdirs: &[],
        matches: is_ps1_bios,
        satisfies: None,
        example: "scph5501.bin or scph1001.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "PS2",
        rom_extensions: &["iso", "chd", "cue", "bin"],
        cores: &["pcsx2"],
        core_subdirs: &["pcsx2/bios"],
        matches: is_ps2_bios,
        satisfies: Some(is_ps2_bios_image),
        example: "SCPH-70012.bin (with its .EROM/.NVM/.ROM1/.ROM2 files)",
        crash_on_missing: true,
    },
    BiosSpec {
        label: "NDS",
        // The classic `melonds` and `desmume` cores read these from system/
        // root; the newer melonDS DS core reads them from its own subdirectory.
        // Placing both costs a few hundred KB and covers either core.
        rom_extensions: &["nds"],
        cores: &["melonds", "desmume"],
        core_subdirs: &["melonDS DS"],
        matches: is_nds_bios,
        satisfies: None,
        example: "bios7.bin, bios9.bin, firmware.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        // Flycast reads its Dreamcast BIOS from `system/dc/`, the same nested
        // convention as PCSX2. Not "crash on launch": recent Flycast can boot
        // some discs with its HLE BIOS, it just misses the boot animation and
        // fails on the titles that need the real one.
        label: "Dreamcast",
        rom_extensions: &["cdi", "gdi"],
        cores: &["flycast", "reicast"],
        core_subdirs: &["dc"],
        matches: is_dreamcast_bios,
        satisfies: None,
        example: "dc_boot.bin and dc_flash.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "Sega CD",
        rom_extensions: &["chd"],
        cores: &["genesis_plus_gx", "picodrive", "blastem"],
        core_subdirs: &[],
        matches: is_segacd_bios,
        satisfies: None,
        example: "bios_cd_u.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "Saturn",
        rom_extensions: &["chd"],
        cores: &["mednafen_saturn", "beetle_saturn", "yabause", "kronos"],
        core_subdirs: &[],
        matches: is_saturn_bios,
        satisfies: None,
        example: "sega_101.bin or mpr-17933.bin",
        crash_on_missing: false,
    },
    BiosSpec {
        label: "GBA",
        rom_extensions: &["gba"],
        cores: &["mgba", "vba_next", "vbam", "gpsp"],
        core_subdirs: &[],
        matches: is_gba_bios,
        satisfies: None,
        example: "gba_bios.bin",
        crash_on_missing: false,
    },
];

/// True when `spec` describes the system the ROM actually resolved to.
///
/// Specs are selected by ROM extension, and the disc extensions are shared:
/// `.cue`/`.bin`/`.chd` match the PS1, PS2, Sega CD and Saturn rows all at
/// once. Placing files for all of them is harmless, but *warning* for all of
/// them is not — the Big Picture game page turns any warning into a blocking
/// "Launch Failed" dialog, so a PS1 disc that boots fine would report three
/// missing BIOSes. `resolved_core` is the filename of the core
/// `resolve_core_for_rom` picked, lowercased.
///
/// `None` means no core could be resolved, and then every extension match still
/// counts: without a core there is nothing better to go on, and a launch with
/// no core is going to fail anyway.
fn spec_matches_core(spec: &BiosSpec, resolved_core: Option<&str>) -> bool {
    let Some(core) = resolved_core else {
        return true;
    };
    spec.cores.iter().any(|fragment| core.contains(fragment))
}

/// Detects BIOS files relevant to the launching ROM, auto-copies any found in
/// `system/` into the subdirectory the core expects, and returns one warning
/// string per system whose BIOS is still missing.
///
/// `current_rom_ext` is the lowercased extension of the launching ROM (or
/// `None` if unknown — then all checks run).
///
/// `resolved_core` is the lowercased filename of the libretro core the ROM
/// resolved to. Placement runs for every spec the extension matches, but only
/// the resolved core's own system can produce a returned warning; the rest stay
/// in the log. See [`spec_matches_core`].
pub fn check_and_place_bios(
    system_dir: &Path,
    current_rom_ext: Option<&str>,
    resolved_core: Option<&str>,
) -> Vec<String> {
    let mut warnings = Vec::new();

    for spec in BIOS_SPECS {
        // Skip BIOS checks irrelevant to the current game.
        if !spec.rom_extensions.is_empty()
            && let Some(ext) = current_rom_ext
                && !spec.rom_extensions.contains(&ext) {
                    continue;
                }

        // Mirror whatever is in `system/` into every subdirectory a core reads
        // from. Cores that read the root need no placement pass at all.
        for sub in spec.core_subdirs {
            place_bios_in_subdir(system_dir, &system_dir.join(sub), spec);
        }

        // Every place this system's BIOS is allowed to live. The root is
        // included whether or not the core reads it, because it is where the
        // user puts files and where the mirror above copies from; a user who
        // instead placed the BIOS straight into the core's subdirectory is
        // covered by the rest.
        let search_dirs: Vec<PathBuf> = std::iter::once(system_dir.to_path_buf())
            .chain(spec.core_subdirs.iter().map(|sub| system_dir.join(sub)))
            .collect();
        let satisfies = spec.satisfies.unwrap_or(spec.matches);

        if let Some(found) = search_dirs.iter().find(|d| dir_has_bios(d, satisfies)) {
            info!("[RETROARCH] {} BIOS found in {}", spec.label, found.display());
            continue;
        }

        // Distinguish "nothing there" from "the companion files are there but
        // the bootable image is not" — the second is what a partial PCSX2 dump
        // looks like, and "place a BIOS" is unhelpful advice for it.
        let has_partial = spec.satisfies.is_some()
            && search_dirs.iter().any(|d| dir_has_bios(d, spec.matches));
        let action = if spec.crash_on_missing {
            "will crash on launch"
        } else {
            "may not boot correctly"
        };
        // The warning always names `system/`, never a core subdirectory: that
        // is the one path the user has to get right, and Drop copies from
        // there into wherever the core actually looks.
        let msg = if has_partial {
            format!(
                "The {} BIOS in {} is incomplete. The companion files are there but the BIOS \
                 image itself is missing. {} games {action}. Add {}",
                spec.label,
                system_dir.display(),
                spec.label,
                spec.example
            )
        } else {
            format!(
                "No {} BIOS found. {} games {action}. Place {} in {}",
                spec.label,
                spec.label,
                spec.example,
                system_dir.display()
            )
        };
        warn!("[RETROARCH] {msg}");
        if spec_matches_core(spec, resolved_core) {
            warnings.push(msg);
        } else {
            info!(
                "[RETROARCH] Not surfacing the {} BIOS warning: this ROM resolved to {}",
                spec.label,
                resolved_core.unwrap_or("an unknown core")
            );
        }
    }

    warnings
}

/// Copies every file in `system_dir` belonging to `spec`'s BIOS set into
/// `target_dir`, the subdirectory the core actually reads.
///
/// **Copy, not hardlink or symlink.** A hardlink saves the few MB but is still
/// an unlink target for the manifest-reconcile sweep in
/// `games::downloads::download_agent`, so it buys nothing there, and it fails
/// outright across filesystems (`system/` on the internal drive, an install on
/// the Deck's SD card). A symlink needs Developer Mode on Windows and is one
/// more thing for Wine to resolve. A plain copy is an ordinary file to every
/// core, on every platform, under Proton or not. The sweep is taught to leave
/// `system/` alone — see `PROTECTED_DATA_DIRS` — which is what actually keeps
/// these files alive across a re-download.
fn place_bios_in_subdir(system_dir: &Path, target_dir: &Path, spec: &BiosSpec) {
    let root_bios: Vec<_> = fs::read_dir(system_dir)
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter(|e| (spec.matches)(&e.file_name().to_string_lossy().to_lowercase()))
        .collect();

    if root_bios.is_empty() {
        return;
    }

    if let Err(e) = fs::create_dir_all(target_dir) {
        warn!(
            "[RETROARCH] Failed to create {} BIOS dir {}: {e}",
            spec.label,
            target_dir.display()
        );
        return;
    }

    info!(
        "[RETROARCH] Found {} {} BIOS file(s) in system/ — placing in {}",
        root_bios.len(),
        spec.label,
        target_dir.display()
    );
    // Every file in the set, every launch: a dump whose image was copied by an
    // older Drop but whose `.NVM`/`.ROM1` companions were not would otherwise
    // stay half-placed forever, and PCSX2 reads the companions from beside the
    // image.
    for entry in &root_bios {
        let dest = target_dir.join(entry.file_name());
        if dest.exists() {
            continue;
        }
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

/// Returns `true` if `dir` contains a file matching `matches`.
fn dir_has_bios(dir: &Path, matches: fn(&str) -> bool) -> bool {
    fs::read_dir(dir)
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .any(|e| matches(&e.file_name().to_string_lossy().to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::{is_ps1_bios, is_ps2_bios, is_ps2_bios_image, spec_matches_core, BIOS_SPECS};

    fn spec(label: &str) -> &'static super::BiosSpec {
        BIOS_SPECS.iter().find(|s| s.label == label).expect("spec exists")
    }

    /// The ground-truth Deck case: Castlevania SOTN is a PS1 disc, and its
    /// `.cue`/`.bin`/`.chd` extension also selects the PS2, Sega CD and Saturn
    /// rows. Only the core that actually loaded it may warn — the other three
    /// used to reach Big Picture as a blocking "Launch Failed" dialog over a
    /// game that was booting fine.
    #[test]
    fn a_ps1_disc_only_warns_about_ps1() {
        let core = Some("swanstation_libretro.dll");
        assert!(spec_matches_core(spec("PS1"), core));
        for other in ["PS2", "Sega CD", "Saturn"] {
            assert!(!spec_matches_core(spec(other), core), "{other} must not warn for a PS1 disc");
        }
    }

    /// A real PS2 disc still gets its warning — scoping must not silence the
    /// case the check exists for.
    #[test]
    fn a_ps2_disc_still_warns_about_ps2() {
        let core = Some("pcsx2_libretro.so");
        assert!(spec_matches_core(spec("PS2"), core));
        assert!(!spec_matches_core(spec("PS1"), core));
    }

    /// No core resolved means no better information than the extension, so
    /// every extension match still counts.
    #[test]
    fn an_unresolved_core_keeps_every_warning() {
        for label in ["PS1", "PS2", "Sega CD", "Saturn", "GBA"] {
            assert!(spec_matches_core(spec(label), None), "{label} should warn without a core");
        }
    }

    /// Core names carry suffixes and variants (`_hw`, `_libretro`, the platform
    /// extension), so the match is on a fragment, not equality.
    #[test]
    fn core_matching_tolerates_name_variants() {
        assert!(spec_matches_core(spec("PS1"), Some("mednafen_psx_hw_libretro.dll")));
        assert!(spec_matches_core(spec("Dreamcast"), Some("flycast_libretro.so")));
        assert!(spec_matches_core(spec("NDS"), Some("melonds_libretro.dll")));
    }

    /// The user's actual Deck dump. The old `.bin`-only rule left every one of
    /// these behind in `system/`, so PCSX2 never saw them.
    #[test]
    fn a_pcsx2_dump_set_is_matched_whole() {
        for name in [
            "scph-70004_bios_v12_pal_200.bin",
            "scph-70004_bios_v12_pal_200.erom",
            "scph-70004_bios_v12_pal_200.nvm",
            "scph-70004_bios_v12_pal_200.rom1",
            "scph-70004_bios_v12_pal_200.rom2",
        ] {
            assert!(is_ps2_bios(name), "{name} should be a PS2 BIOS file");
        }
    }

    /// Only the image counts as "a PS2 BIOS is present" — the companions
    /// cannot boot on their own, and treating them as enough is what would
    /// suppress the warning for a half-copied dump.
    #[test]
    fn only_the_image_satisfies_the_ps2_check() {
        assert!(is_ps2_bios_image("scph-70004_bios_v12_pal_200.bin"));
        assert!(!is_ps2_bios_image("scph-70004_bios_v12_pal_200.nvm"));
    }

    /// PS1 BIOS images share the `scph` prefix. Counting them as PS2 BIOS was
    /// what silenced the "no PS2 BIOS" warning on a machine that had none.
    #[test]
    fn ps1_bioses_are_not_ps2_bioses() {
        for name in ["scph5500.bin", "scph5501.bin", "scph5502.bin", "scph1001.bin"] {
            assert!(!is_ps2_bios(name), "{name} is a PS1 BIOS");
        }
        assert!(is_ps1_bios("scph5501.bin"));
    }

    #[test]
    fn unrelated_files_are_not_bios() {
        for name in ["openbios.bin", "readme.txt", "pcsx2", "gba_bios.bin"] {
            assert!(!is_ps2_bios(name), "{name} should not match PS2");
        }
    }
}

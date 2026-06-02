//! ROM file-extension → libretro core mapping and core resolution.
//!
//! # The mapping is data-driven
//!
//! [`EXTENSION_CORE_MAP`] is the **single source of truth** for everything
//! the rest of the RetroArch module needs to know about a ROM extension:
//!
//! * which libretro cores can run it (in preference order),
//! * its [`RenderClass`] — low-res 2D vs high-res 3D — which decides whether
//!   CRT shaders are safe and which controller-device quirks apply.
//!
//! Adding support for a new platform is one line in the table; the
//! high-res-3D and Dolphin heuristics fall out of it automatically. There is
//! deliberately **no** giant hand-written `match ext { ... }` duplicating this
//! data — the old code had three of them and they drifted apart.
//!
//! # Ambiguous extensions
//!
//! `.iso` can be a GameCube/Wii disc (Dolphin) or a PS2/PSP/Saturn disc.
//! `resolve_core_for_rom` reads the disc header magic to disambiguate before
//! choosing a core — see [`detect_iso_disc_type`].

use log::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};

/// How a platform renders, which drives shader / controller decisions.
///
/// * [`RenderClass::Retro2D`] — low-res 2D consoles (NES, SNES, Game Boy,
///   Genesis, …). CRT shaders look great; native source is ~256-512px wide.
/// * [`RenderClass::HighRes3D`] — 3D consoles that Drop's quality presets
///   upscale well past where slang CRT shaders work (Dolphin, PCSX2, PPSSPP,
///   N64, Dreamcast, 3DS, DS, and the first-gen-3D disc systems).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderClass {
    Retro2D,
    HighRes3D,
}

/// One row of the ROM-extension → core table.
#[derive(Debug, Clone, Copy)]
pub struct ExtensionEntry {
    /// Lowercase ROM file extension, no dot (e.g. `"gba"`).
    pub ext: &'static str,
    /// libretro core name fragments to search for, in priority order.
    /// Core filenames look like `mgba_libretro.dll` / `dolphin_libretro.so`.
    pub cores: &'static [&'static str],
    /// Render class — drives the CRT-shader and controller-device heuristics.
    pub render: RenderClass,
}

use RenderClass::{HighRes3D, Retro2D};

/// **The** ROM-extension → core table. See module docs.
///
/// `.iso` and `.chd` are intrinsically ambiguous; their rows carry a
/// best-effort fallback core order, but [`resolve_core_for_rom`] sniffs the
/// disc header for `.iso` before trusting it.
pub const EXTENSION_CORE_MAP: &[ExtensionEntry] = &[
    // ── Nintendo handhelds / 2D ──────────────────────────────────────────
    ExtensionEntry { ext: "gba", cores: &["mgba", "vba_next", "gpsp"], render: Retro2D },
    ExtensionEntry { ext: "gbc", cores: &["gambatte", "mgba", "sameboy"], render: Retro2D },
    ExtensionEntry { ext: "gb", cores: &["gambatte", "mgba", "sameboy"], render: Retro2D },
    ExtensionEntry { ext: "nes", cores: &["mesen", "nestopia", "fceumm", "quicknes"], render: Retro2D },
    ExtensionEntry { ext: "fds", cores: &["mesen", "nestopia", "fceumm"], render: Retro2D },
    ExtensionEntry { ext: "sfc", cores: &["snes9x", "bsnes", "mesen-s"], render: Retro2D },
    ExtensionEntry { ext: "smc", cores: &["snes9x", "bsnes", "mesen-s"], render: Retro2D },
    // ── Nintendo 3D consoles ─────────────────────────────────────────────
    ExtensionEntry { ext: "n64", cores: &["mupen64plus_next", "mupen64plus", "parallel_n64"], render: HighRes3D },
    ExtensionEntry { ext: "z64", cores: &["mupen64plus_next", "mupen64plus", "parallel_n64"], render: HighRes3D },
    ExtensionEntry { ext: "v64", cores: &["mupen64plus_next", "mupen64plus", "parallel_n64"], render: HighRes3D },
    ExtensionEntry { ext: "nds", cores: &["melonds", "desmume"], render: HighRes3D },
    ExtensionEntry { ext: "3ds", cores: &["citra"], render: HighRes3D },
    // GameCube / Wii — all Dolphin, all high-res 3D.
    ExtensionEntry { ext: "gcm", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "gcz", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "rvz", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "wbfs", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "wad", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "dol", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "elf", cores: &["dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "wia", cores: &["dolphin"], render: HighRes3D },
    // .iso — ambiguous, resolved at runtime by detect_iso_disc_type().
    // Fallback if disc detection fails: pcsx2 > dolphin (PS2 ISOs far commoner).
    ExtensionEntry { ext: "iso", cores: &["pcsx2", "dolphin", "ppsspp", "mednafen_saturn"], render: HighRes3D },
    // ── Sony ─────────────────────────────────────────────────────────────
    ExtensionEntry { ext: "cue", cores: &["beetle_psx", "mednafen_psx", "pcsx_rearmed", "swanstation", "pcsx2"], render: HighRes3D },
    ExtensionEntry { ext: "bin", cores: &["beetle_psx", "mednafen_psx", "pcsx_rearmed", "genesis_plus_gx"], render: HighRes3D },
    ExtensionEntry { ext: "chd", cores: &["beetle_psx", "mednafen_psx", "swanstation", "pcsx2", "dolphin"], render: HighRes3D },
    ExtensionEntry { ext: "pbp", cores: &["beetle_psx", "pcsx_rearmed", "ppsspp"], render: HighRes3D },
    ExtensionEntry { ext: "cso", cores: &["ppsspp"], render: HighRes3D },
    ExtensionEntry { ext: "psp", cores: &["ppsspp"], render: HighRes3D },
    // ── Sega ─────────────────────────────────────────────────────────────
    ExtensionEntry { ext: "md", cores: &["genesis_plus_gx", "picodrive", "blastem"], render: Retro2D },
    ExtensionEntry { ext: "gen", cores: &["genesis_plus_gx", "picodrive", "blastem"], render: Retro2D },
    ExtensionEntry { ext: "smd", cores: &["genesis_plus_gx", "picodrive"], render: Retro2D },
    ExtensionEntry { ext: "32x", cores: &["picodrive"], render: Retro2D },
    ExtensionEntry { ext: "sms", cores: &["genesis_plus_gx", "picodrive"], render: Retro2D },
    ExtensionEntry { ext: "gg", cores: &["genesis_plus_gx"], render: Retro2D },
    // Dreamcast — Flycast, 3D.
    ExtensionEntry { ext: "cdi", cores: &["flycast"], render: HighRes3D },
    ExtensionEntry { ext: "gdi", cores: &["flycast"], render: HighRes3D },
    // ── Atari ────────────────────────────────────────────────────────────
    ExtensionEntry { ext: "a26", cores: &["stella"], render: Retro2D },
    ExtensionEntry { ext: "a78", cores: &["prosystem"], render: Retro2D },
    ExtensionEntry { ext: "lnx", cores: &["mednafen_lynx", "handy"], render: Retro2D },
    // ── Other 2D ─────────────────────────────────────────────────────────
    ExtensionEntry { ext: "pce", cores: &["beetle_pce", "mednafen_pce"], render: Retro2D },
    ExtensionEntry { ext: "ngp", cores: &["beetle_ngp", "mednafen_ngp"], render: Retro2D },
    ExtensionEntry { ext: "ngc", cores: &["beetle_ngp", "mednafen_ngp"], render: Retro2D },
    ExtensionEntry { ext: "ws", cores: &["beetle_wswan", "mednafen_wswan"], render: Retro2D },
    ExtensionEntry { ext: "wsc", cores: &["beetle_wswan", "mednafen_wswan"], render: Retro2D },
    ExtensionEntry { ext: "vec", cores: &["vecx"], render: Retro2D },
    ExtensionEntry { ext: "col", cores: &["bluemsx"], render: Retro2D },
];

/// Looks up the table row for a ROM extension (case-insensitive).
pub fn entry_for_ext(ext: &str) -> Option<&'static ExtensionEntry> {
    let lower = ext.to_lowercase();
    EXTENSION_CORE_MAP.iter().find(|e| e.ext == lower)
}

/// Extracts the lowercase extension from a ROM path.
fn rom_ext(rom_path: &str) -> Option<String> {
    Path::new(rom_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
}

/// True when the ROM will be loaded by Dolphin (GameCube/Wii).
///
/// Derived from the table: any extension whose only candidate core is
/// `dolphin`. Used to apply Dolphin-specific shader-compat workarounds.
///
/// `.iso` is intentionally excluded — it is more often a PS2 disc than a Wii
/// disc, and PCSX2's shader pipeline already works; the disc sniff in
/// `resolve_core_for_rom` handles the genuine Wii-ISO case separately.
pub fn rom_uses_dolphin_core(rom_path: &str) -> bool {
    let Some(ext) = rom_ext(rom_path) else { return false };
    if ext == "iso" {
        return false;
    }
    entry_for_ext(&ext)
        .map(|e| e.cores == ["dolphin"])
        .unwrap_or(false)
}

/// True when the ROM resolves to a core that renders at high internal
/// resolution / native 3D, where slang CRT shaders break video output.
///
/// Derived directly from the table's [`RenderClass`] — no duplicated match.
/// `configure_retroarch_for_game` uses this to pick a resolution-tolerant CRT
/// shader (or suppress the strong scanline shader) for these ROMs.
pub fn rom_implies_high_res_3d_core(rom_path: &str) -> bool {
    let Some(ext) = rom_ext(rom_path) else { return false };
    entry_for_ext(&ext)
        .map(|e| e.render == RenderClass::HighRes3D)
        .unwrap_or(false)
}

/// Disc type detected from an ISO file's header magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsoDiscType {
    /// GameCube disc — magic word `0xC2339F3D` at offset `0x1C`.
    GameCube,
    /// Wii disc — magic word `0x5D1C9EA3` at offset `0x18`.
    Wii,
    /// Not a Nintendo disc — likely PS2, PSP, or Saturn.
    Other,
}

/// Reads the first 32 bytes of an ISO and checks for Nintendo disc magic
/// words to tell GameCube/Wii ISOs apart from PS2/PSP/Saturn ISOs.
pub fn detect_iso_disc_type(iso_path: &Path) -> IsoDiscType {
    use std::io::Read;

    let mut buf = [0u8; 32];
    let Ok(mut f) = fs::File::open(iso_path) else {
        warn!("[RETROARCH] Cannot open ISO for disc detection: {}", iso_path.display());
        return IsoDiscType::Other;
    };
    if f.read_exact(&mut buf).is_err() {
        warn!("[RETROARCH] ISO too small for disc detection: {}", iso_path.display());
        return IsoDiscType::Other;
    }

    // Wii magic: 0x5D1C9EA3 at offset 0x18.
    let wii_magic = u32::from_be_bytes([buf[0x18], buf[0x19], buf[0x1A], buf[0x1B]]);
    if wii_magic == 0x5D1C_9EA3 {
        info!("[RETROARCH] ISO disc detection: Wii disc — {}", iso_path.display());
        return IsoDiscType::Wii;
    }

    // GameCube magic: 0xC2339F3D at offset 0x1C.
    let gc_magic = u32::from_be_bytes([buf[0x1C], buf[0x1D], buf[0x1E], buf[0x1F]]);
    if gc_magic == 0xC233_9F3D {
        info!("[RETROARCH] ISO disc detection: GameCube disc — {}", iso_path.display());
        return IsoDiscType::GameCube;
    }

    info!("[RETROARCH] ISO disc detection: non-Nintendo disc — {}", iso_path.display());
    IsoDiscType::Other
}

/// Given a ROM path and the RetroArch install directory, find a matching
/// core in `<emu_root>/cores/`. Returns the absolute path to the core
/// library, or `None` if no match.
///
/// `.m3u` playlists resolve via their first disc entry. `.iso` files have
/// their disc header sniffed so a PS2 ISO doesn't get loaded into Dolphin.
pub fn resolve_core_for_rom(emu_root: &Path, rom_path: &str) -> Option<PathBuf> {
    let rom = Path::new(rom_path);
    let rom_ext = rom
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())?;

    // .m3u playlists (multi-disc): resolve the core from the first disc.
    if rom_ext == "m3u" {
        if let Ok(contents) = fs::read_to_string(rom)
            && let Some(first_line) = contents
                .lines()
                .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
            {
                let first_disc = first_line.trim();
                info!("[RETROARCH] .m3u playlist — resolving core from first disc: {first_disc}");
                return resolve_core_for_rom(emu_root, first_disc);
            }
        warn!("[RETROARCH] .m3u file is empty or unreadable: {rom_path}");
        return None;
    }

    // For .iso, sniff the disc header to pick the right core. Without this the
    // static table order would always win (previously Dolphin, breaking PS2).
    let iso_candidates: &[&str];
    let candidates: &[&str] = if rom_ext == "iso" {
        iso_candidates = match detect_iso_disc_type(rom) {
            IsoDiscType::GameCube | IsoDiscType::Wii => &["dolphin"],
            IsoDiscType::Other => &["pcsx2", "ppsspp", "mednafen_saturn", "dolphin"],
        };
        iso_candidates
    } else {
        entry_for_ext(&rom_ext)?.cores
    };

    let cores_dir = emu_root.join("cores");
    if !cores_dir.is_dir() {
        warn!("[RETROARCH] cores/ directory not found in {}", emu_root.display());
        return None;
    }

    let entries: Vec<_> = fs::read_dir(&cores_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .contains("_libretro")
        })
        .collect();

    for candidate in candidates {
        let needle = candidate.to_lowercase();
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.contains(&needle) && name.contains("_libretro") {
                let core_path = entry.path();
                info!(
                    "[RETROARCH] Resolved core for .{rom_ext}: {} (matched '{candidate}')",
                    core_path.display()
                );
                return Some(core_path);
            }
        }
    }

    warn!(
        "[RETROARCH] No core found for .{rom_ext} extension in {}",
        cores_dir.display()
    );
    None
}

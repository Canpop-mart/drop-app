//! Quality-preset and aspect-ratio config.
//!
//! Drop exposes a quality preset (Low/Medium/High/Ultra) and an aspect ratio
//! per emulated game. These map onto two RetroArch config files:
//!
//! * `retroarch.cfg` — frontend-level video settings (smoothing, integer
//!   scale, aspect index). Handled by [`apply_quality_preset`] /
//!   [`apply_widescreen`].
//! * `retroarch-core-options.cfg` — per-core internal-resolution and
//!   widescreen-hack options. Handled by [`apply_core_quality_options`] /
//!   [`apply_core_widescreen_options`].
//!
//! # Why every value here is pinned to the core's own option table
//!
//! RetroArch gives no failure signal for a bad core option. It keeps a
//! rejected *key* in `retroarch-core-options.cfg` verbatim and silently swaps
//! a rejected *value* for the core default, logging nothing. So a setting
//! being present in the file proves nothing, and a guessed value is
//! indistinguishable from a working one until someone looks at the screen.
//!
//! Every key and value below therefore comes from the core's own
//! `libretro_core_options.h` (or, for Dolphin, `DolphinLibretro/Common/
//! Options.cpp`) — never from the RetroArch menu labels, which are display
//! text and usually differ from the value string the core accepts. Several of
//! the corrections in this file exist precisely because Drop used to write the
//! label.
//!
//! # Only installed cores are written
//!
//! Keys are gated on the core actually being present in `<emu_root>/cores/`
//! ([`InstalledCores`]). Writing options for cores that never shipped left the
//! config file — and Drop's own log — claiming settings nothing would read.

use database::models::data::{AspectRatio, QualityPreset};
use log::{info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ── Quality preset (retroarch.cfg) ───────────────────────────────────────

/// Applies frontend video settings for the quality preset to `retroarch.cfg`.
///
/// `video_scale` only affects windowed mode, so in fullscreen the levers are
/// `video_smooth`, `video_scale_integer` and `video_frame_delay`. The real
/// internal-resolution work happens in [`apply_core_quality_options`].
///
/// Deliberately does **not** touch `video_shader*` — shader state is owned
/// entirely by the CRT toggle, which runs after this.
pub fn apply_quality_preset(overrides: &mut HashMap<&str, String>, quality: &QualityPreset) {
    let (smooth, scale_integer, gpu_screenshot, frame_delay) = match quality {
        QualityPreset::Low => ("false", "false", "false", "0"),
        QualityPreset::Medium => ("true", "false", "true", "0"),
        QualityPreset::High => ("true", "true", "true", "4"),
        // Max frame delay reduces input lag on powerful hardware.
        QualityPreset::Ultra => ("true", "true", "true", "8"),
    };
    overrides.insert("video_smooth", smooth.into());
    overrides.insert("video_scale_integer", scale_integer.into());
    overrides.insert("video_gpu_screenshot", gpu_screenshot.into());
    overrides.insert("video_frame_delay", frame_delay.into());
}

// ── Aspect ratio (retroarch.cfg) ─────────────────────────────────────────

/// Applies the aspect ratio to `retroarch.cfg`.
///
/// RetroArch aspect indices: `0=4:3 1=16:9 2=16:10 22=Core provided`.
pub fn apply_widescreen(overrides: &mut HashMap<&str, String>, ratio: &AspectRatio) {
    match ratio {
        AspectRatio::Standard => {
            overrides.insert("aspect_ratio_index", "22".into());
            overrides.insert("video_aspect_ratio_auto", "true".into());
        }
        AspectRatio::Wide16_9 => {
            overrides.insert("aspect_ratio_index", "1".into());
            overrides.insert("video_aspect_ratio_auto", "false".into());
        }
        AspectRatio::Wide16_10 => {
            overrides.insert("aspect_ratio_index", "2".into());
            overrides.insert("video_aspect_ratio_auto", "false".into());
        }
    }
}

// ── Which cores are actually installed ───────────────────────────────────

/// The libretro cores present in `<emu_root>/cores/` — the directory Drop
/// points `libretro_directory` at, so it is the only place RetroArch loads
/// cores from.
///
/// Scanned once per launch and passed to both core-option passes so a block
/// for a core that never shipped is skipped instead of writing dead keys.
pub struct InstalledCores {
    /// Lowercased file names of everything in `cores/` that looks like a core.
    names: Vec<String>,
}

impl InstalledCores {
    /// Reads `<emu_root>/cores/`. A missing or unreadable directory yields an
    /// empty set, which correctly suppresses every core-option block — there
    /// are no cores to configure.
    pub fn scan(emu_root: &Path) -> Self {
        let cores_dir = emu_root.join("cores");
        let Ok(entries) = fs::read_dir(&cores_dir) else {
            warn!(
                "[RETROARCH] Cannot read {} — skipping all core options",
                cores_dir.display()
            );
            return Self { names: Vec::new() };
        };
        let names = entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_lowercase())
            .filter(|n| n.contains("_libretro"))
            .collect();
        Self { names }
    }

    /// Builds the set directly from core file names, so the gating logic can
    /// be exercised without a RetroArch install on disk.
    #[cfg(test)]
    fn from_names<I, S>(names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self {
            names: names.into_iter().map(|n| n.as_ref().to_lowercase()).collect(),
        }
    }

    /// True when a core whose file name contains `fragment` is installed.
    ///
    /// Substring matching, the same convention [`super::cores`] uses to
    /// resolve a core for a ROM: `"mupen64plus"` matches the shipped
    /// `mupen64plus_next_libretro.so`, and the platform suffix (`.so`, `.dll`,
    /// `.dylib`) never has to be spelled out.
    ///
    /// `fragment` must be lowercase.
    pub fn has(&self, fragment: &str) -> bool {
        self.names.iter().any(|n| n.contains(fragment))
    }
}

// ── Core options (retroarch-core-options.cfg) ────────────────────────────

/// Internal-resolution targets for one quality preset, one per core family.
///
/// The N64 cores need both a 4:3 and a 16:9 target because mupen64plus-next
/// keeps a separate resolution list per aspect and picks between them at
/// runtime from `mupen64plus-aspect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolutionTargets {
    /// `dolphin_efb_scale` — native multiplier, "1".."6".
    dolphin_efb: &'static str,
    /// `mupen64plus-43screensize` / `parallel-n64-screensize`.
    n64_res_43: &'static str,
    /// `mupen64plus-169screensize`.
    n64_res_169: &'static str,
    /// `beetle_psx_hw_internal_resolution`.
    beetle_psx_res: &'static str,
    /// `ppsspp_internal_resolution` — a WxH string, not a multiplier.
    ppsspp_res: &'static str,
}

/// Maps a quality preset onto each core's internal-resolution option.
///
/// Every value is a member of that option's value list in the core's own
/// option table; the unit tests below assert exactly that.
fn resolution_targets(quality: &QualityPreset) -> ResolutionTargets {
    let (dolphin_efb, n64_res_43, n64_res_169, beetle_psx_res, ppsspp_res) = match quality {
        QualityPreset::Low => ("1", "320x240", "640x360", "1x(native)", "480x272"),
        QualityPreset::Medium => ("3", "640x480", "1280x720", "2x", "960x544"),
        QualityPreset::High => ("5", "1280x960", "1920x1080", "4x", "1920x1088"),
        QualityPreset::Ultra => ("6", "1920x1440", "2560x1440", "8x", "3840x2176"),
    };
    ResolutionTargets { dolphin_efb, n64_res_43, n64_res_169, beetle_psx_res, ppsspp_res }
}

/// Applies per-core internal-resolution and image-quality options for the
/// quality preset to `retroarch-core-options.cfg`.
///
/// Each core's block is skipped unless that core is installed, so the returned
/// key set — and the log line the caller writes from it — only ever describes
/// settings a core will actually read.
pub fn apply_core_quality_options(
    overrides: &mut HashMap<&str, String>,
    quality: &QualityPreset,
    cores: &InstalledCores,
) {
    let res = resolution_targets(quality);
    let mut written: Vec<&str> = Vec::new();

    // Dolphin (GameCube/Wii).
    //
    // `dolphin_anti_aliasing` takes a bare index, not the label: the core's
    // option table is values "0".."6" against labels None / 2x MSAA / 4x MSAA /
    // 8x MSAA / 2x SSAA / 4x SSAA / 8x SSAA. Drop used to write the labels,
    // which the core rejects, so every preset silently ran with no AA. Ultra
    // stops at 8x MSAA rather than climbing into SSAA — supersampling renders
    // the whole frame at 2-8x and would tank the frame rate on a Deck-class GPU
    // on top of the EFB-scale increase the preset already applies.
    //
    // Same story for anisotropy and forced texture filtering: they used to be
    // written as `dolphin_anisotropic_filtering` and
    // `dolphin_force_texture_filtering`, neither of which exists in the core,
    // so they were silent no-ops for their whole life. The real keys are
    // `dolphin_max_anisotropy` (values "0".."4" against labels 1x (Off) / 2x /
    // 4x / 8x / 16x) and `dolphin_force_texture_filtering_mode` (values "0".."2"
    // against Default / Nearest (Sharp) / Linear (Smooth)).
    if cores.has("dolphin") {
        written.push("dolphin");
        overrides.insert("dolphin_efb_scale", quoted(res.dolphin_efb));

        let (dolphin_aa, dolphin_efb_access) = match quality {
            QualityPreset::Low => ("0", "disabled"),   // None
            QualityPreset::Medium => ("1", "enabled"), // 2x MSAA
            QualityPreset::High => ("2", "enabled"),   // 4x MSAA
            QualityPreset::Ultra => ("3", "enabled"),  // 8x MSAA
        };
        overrides.insert("dolphin_anti_aliasing", quoted(dolphin_aa));
        overrides.insert("dolphin_efb_access_enable", quoted(dolphin_efb_access));

        // Renderer — hygiene, not a fix. "Hardware" is the option's only legal
        // value in a release build (Software/Null are compiled out), so this
        // just pins the key to it and puts it in the file the core actually
        // reads. Drop used to write it into retroarch.cfg, where it sat unread.
        // Note this is the rasteriser, not the graphics API — the API is
        // retroarch.cfg's video_driver.
        overrides.insert("dolphin_renderer", "\"Hardware\"".into());

        // Wii in particular needs more than EFB scale to look right at modern
        // resolutions: anisotropic filtering keeps oblique-angle textures sharp,
        // forced texture filtering smooths nearest-neighbour textures (skipped
        // on Low to preserve the original look and the perf), and progressive
        // scan avoids the interlace artifacts PAL/JP titles ship with.
        let dolphin_aniso = match quality {
            QualityPreset::Low => "0",    // 1x (off)
            QualityPreset::Medium => "1", // 2x
            QualityPreset::High => "2",   // 4x
            QualityPreset::Ultra => "4",  // 16x
        };
        overrides.insert("dolphin_max_anisotropy", quoted(dolphin_aniso));
        overrides.insert(
            "dolphin_force_texture_filtering_mode",
            // 0 = Default (leave the game's own filtering alone), 2 = Linear.
            quoted(if matches!(quality, QualityPreset::Low) { "0" } else { "2" }),
        );
        overrides.insert("dolphin_progressive_scan", "\"enabled\"".into());
    }

    // Mupen64Plus-Next (N64).
    //
    // There is no `mupen64plus-Resolution`; Drop wrote one for years and the
    // core dropped it every time. The core keeps two independent resolution
    // lists and chooses between them at runtime from `mupen64plus-aspect`, so
    // both have to be set or switching to widescreen would silently fall back
    // to the 16:9 list's default.
    //
    // 6xBRZ belongs to `txEnhancementMode`, not `txFilterMode` — writing it to
    // the latter (whose list is None / Smooth filtering 1-4 / Sharp filtering
    // 1-2) meant High and Ultra ran with no texture filtering at all.
    if cores.has("mupen64plus") {
        written.push("mupen64plus");
        overrides.insert("mupen64plus-43screensize", quoted(res.n64_res_43));
        overrides.insert("mupen64plus-169screensize", quoted(res.n64_res_169));

        let (n64_txfilter, n64_txenhance) = match quality {
            QualityPreset::Low | QualityPreset::Medium => ("None", "None"),
            QualityPreset::High | QualityPreset::Ultra => ("Smooth filtering 3", "6xBRZ"),
        };
        overrides.insert("mupen64plus-txFilterMode", quoted(n64_txfilter));
        overrides.insert("mupen64plus-txEnhancementMode", quoted(n64_txenhance));
        // mupen64plus-aspect is deliberately not written here — the widescreen
        // pass runs after this one on the same map and owns that key.
    }

    // ParaLLEl N64 — 4:3 resolution list only; widescreen is a separate hint.
    if cores.has("parallel_n64") {
        written.push("parallel_n64");
        overrides.insert("parallel-n64-screensize", quoted(res.n64_res_43));
    }

    // PCSX ReARMed (PS1) — NEON enhancement doubles the internal resolution.
    if cores.has("pcsx_rearmed") {
        written.push("pcsx_rearmed");
        overrides.insert(
            "pcsx_rearmed_neon_enhancement_enable",
            quoted(if matches!(quality, QualityPreset::Low) { "disabled" } else { "enabled" }),
        );
        // The speed hack trades compatibility for frame rate; never worth it.
        overrides.insert("pcsx_rearmed_neon_enhancement_no_main", "\"disabled\"".into());
    }

    // Beetle PSX HW (PS1) — internal resolution, dithering, filtering, PGXP.
    //
    // Only 1x carries the "(native)" suffix; 2x/4x/8x are bare. PGXP is a
    // three-way (disabled / memory only / memory + CPU), not a boolean —
    // "enabled" was rejected, so PGXP never ran on any preset.
    if cores.has("beetle_psx_hw") {
        written.push("beetle_psx_hw");
        overrides.insert("beetle_psx_hw_internal_resolution", quoted(res.beetle_psx_res));

        let (psx_dither, psx_filter, psx_pgxp) = match quality {
            QualityPreset::Low => ("1x(native)", "nearest", "disabled"),
            QualityPreset::Medium => ("1x(native)", "nearest", "memory only"),
            QualityPreset::High | QualityPreset::Ultra => ("disabled", "bilinear", "memory + CPU"),
        };
        overrides.insert("beetle_psx_hw_dither_mode", quoted(psx_dither));
        overrides.insert("beetle_psx_hw_filter", quoted(psx_filter));
        overrides.insert("beetle_psx_hw_pgxp_mode", quoted(psx_pgxp));
        // Ultra: PGXP perspective-correct texturing kills PS1 texture warping.
        overrides.insert(
            "beetle_psx_hw_pgxp_texture",
            quoted(if matches!(quality, QualityPreset::Ultra) { "enabled" } else { "disabled" }),
        );
    }

    // SwanStation (PS1) — GPU resolution scale. The key separator is an
    // underscore, not the dot the DuckStation settings tree uses.
    if cores.has("swanstation") {
        written.push("swanstation");
        let ps1_res_scale = match quality {
            QualityPreset::Low => "1",
            QualityPreset::Medium => "2",
            QualityPreset::High => "4",
            QualityPreset::Ultra => "8",
        };
        overrides.insert("swanstation_GPU_ResolutionScale", quoted(ps1_res_scale));
    }

    // PPSSPP (PSP) — internal resolution is a WxH string off a fixed list, not
    // a multiplier, and the scaling type is lowercase ("xbrz", not "xBRZ").
    if cores.has("ppsspp") {
        written.push("ppsspp");
        overrides.insert("ppsspp_internal_resolution", quoted(res.ppsspp_res));

        let ppsspp_texfilter = match quality {
            QualityPreset::Low | QualityPreset::Medium => "Auto",
            QualityPreset::High | QualityPreset::Ultra => "Linear",
        };
        overrides.insert("ppsspp_texture_filtering", quoted(ppsspp_texfilter));
        if matches!(quality, QualityPreset::Ultra) {
            overrides.insert("ppsspp_texture_scaling_type", "\"xbrz\"".into());
            overrides.insert("ppsspp_texture_scaling_level", "\"3x\"".into());
        }
    }

    // mGBA — colour correction. The value list is OFF/GBA/GBC/Auto; "Game Boy
    // Advance" is the menu label for GBA, and the core rejected it.
    if cores.has("mgba") {
        written.push("mgba");
        overrides.insert(
            "mgba_color_correction",
            quoted(if matches!(quality, QualityPreset::Low) { "OFF" } else { "GBA" }),
        );
    }

    // Snes9x — hi-res blending for the games that fake transparency with it.
    if cores.has("snes9x") {
        written.push("snes9x");
        overrides.insert(
            "snes9x_hires_blend",
            quoted(if matches!(quality, QualityPreset::Low) { "disabled" } else { "merge" }),
        );
    }

    // PCSX2 (PS2) — internal-resolution upscale + image quality.
    if cores.has("pcsx2") {
        written.push("pcsx2");
        let (pcsx2_res, pcsx2_aniso, pcsx2_dither, pcsx2_texfilter, pcsx2_blend) = match quality {
            QualityPreset::Low => ("1x Native (PS2)", "disabled", "Unscaled", "Bilinear (PS2)", "Minimum"),
            QualityPreset::Medium => ("2x Native (~720p)", "2x", "Scaled", "Bilinear (PS2)", "Basic"),
            QualityPreset::High => ("4x Native (~1440p/2K)", "8x", "Scaled", "Bilinear (PS2)", "High"),
            QualityPreset::Ultra => ("6x Native (~2160p/4K)", "16x", "disabled", "Bilinear (PS2)", "Full"),
        };
        overrides.insert("pcsx2_upscale_multiplier", quoted(pcsx2_res));
        overrides.insert("pcsx2_anisotropic_filtering", quoted(pcsx2_aniso));
        overrides.insert("pcsx2_dithering", quoted(pcsx2_dither));
        overrides.insert("pcsx2_texture_filtering", quoted(pcsx2_texfilter));
        overrides.insert("pcsx2_blending_accuracy", quoted(pcsx2_blend));

        // "Automatic" is the deinterlacer's own value; "Adaptive" alone is not
        // in the list (it is "Adaptive TFF"/"Adaptive BFF", field order the
        // core can't know up front), so the old write fell back to the default.
        overrides.insert("pcsx2_deinterlace_mode", "\"Automatic\"".into());

        // Keep hardware rendering; "Auto" follows the RetroArch video driver.
        // There is no `pcsx2_renderer_backend` — this is the only renderer key.
        overrides.insert("pcsx2_renderer", "\"Auto\"".into());
        info!("[RETROARCH] PCSX2 core options: renderer=Auto, upscale={pcsx2_res}");
    }

    info!("[RETROARCH] Quality core options written for installed cores: {}", summarise(&written));
}

/// Applies per-core widescreen-hack options to `retroarch-core-options.cfg`.
///
/// Native widescreen hacks are enabled for both 16:9 and 16:10; the hacks
/// render at native wide resolution and [`apply_widescreen`] sets the final
/// display ratio. As with the quality pass, a core that is not installed is
/// skipped rather than written blind.
pub fn apply_core_widescreen_options(
    overrides: &mut HashMap<&str, String>,
    ratio: &AspectRatio,
    cores: &InstalledCores,
) {
    let enabled = !matches!(ratio, AspectRatio::Standard);
    let enabled_disabled = if enabled { "enabled" } else { "disabled" };
    let mut written: Vec<&str> = Vec::new();

    // Dolphin.
    //
    // Three separate settings, all needed:
    //   * widescreen_hack — stretches the GameCube's 4:3 projection to 16:9.
    //     The only widescreen lever GameCube games have.
    //   * aspect_ratio — how the finished image is scaled. Values are bare
    //     indices ("0".."6" against Auto / Force Wide / Force Standard /
    //     Stretch / Custom / Custom Stretch / Raw), not the menu labels.
    //   * widescreen — the Wii SYSCONF widescreen flag, which makes Wii games
    //     render a true anamorphic 16:9 frame rather than a stretched 4:3 one.
    //
    // DELIBERATE BEHAVIOUR CHANGE: aspect_ratio was never written, so it sat on
    // the core default of "3" (Stretch), which distorts the picture to fill the
    // window whatever the user picked. Standard now gets Auto — Dolphin reports
    // the game's real ratio and RetroArch's "Core provided" index honours it —
    // and widescreen gets Force Wide.
    //
    // dolphin_widescreen is set alongside because it and aspect_ratio disagree
    // otherwise: its core default is "enabled", so on Standard a Wii game would
    // render anamorphic 16:9 and Auto would faithfully display that 16:9 — the
    // user asked for 4:3 and would get widescreen. Tying it to the user's choice
    // makes the Wii render natively at the requested ratio, which beats the
    // GameCube-style hack. It is a SYSCONF value read at boot and is inert for
    // GameCube titles, so widescreen_hack still has to be written for those.
    if cores.has("dolphin") {
        written.push("dolphin");
        overrides.insert("dolphin_widescreen_hack", quoted(enabled_disabled));
        overrides.insert("dolphin_aspect_ratio", quoted(if enabled { "1" } else { "0" }));
        overrides.insert("dolphin_widescreen", quoted(enabled_disabled));
    }

    // Mupen64Plus-Next — "16:9 adjusted" is the N64 widescreen hack (it fixes
    // up the game's projection); plain "16:9" only stretches the 4:3 image.
    // This key also picks which of the core's two resolution lists is read, so
    // it has to agree with the 43/169 screensize pair the quality pass wrote.
    if cores.has("mupen64plus") {
        written.push("mupen64plus");
        overrides.insert("mupen64plus-aspect", quoted(if enabled { "16:9 adjusted" } else { "4:3" }));
    }

    // ParaLLEl N64 — the key is `aspectratiohint` and its values are words.
    if cores.has("parallel_n64") {
        written.push("parallel_n64");
        overrides.insert(
            "parallel-n64-aspectratiohint",
            quoted(if enabled { "widescreen" } else { "normal" }),
        );
    }

    // Beetle PSX HW — the hack's ratio option has no 4:3 member, so on Standard
    // the hack is simply switched off and the ratio key left alone.
    if cores.has("beetle_psx_hw") {
        written.push("beetle_psx_hw");
        overrides.insert("beetle_psx_hw_widescreen_hack", quoted(enabled_disabled));
        match ratio {
            AspectRatio::Standard => {}
            AspectRatio::Wide16_9 => {
                overrides.insert("beetle_psx_hw_widescreen_hack_aspect_ratio", "\"16:9\"".into());
            }
            AspectRatio::Wide16_10 => {
                overrides.insert("beetle_psx_hw_widescreen_hack_aspect_ratio", "\"16:10\"".into());
            }
        }
    }

    // SwanStation — underscore key, boolean-word values.
    if cores.has("swanstation") {
        written.push("swanstation");
        overrides.insert(
            "swanstation_GPU_WidescreenHack",
            quoted(if enabled { "true" } else { "false" }),
        );
    }

    // PCSX2 — the hint carries the target ratio in the value, and it applies a
    // widescreen patch from the core's internal database rather than a hack.
    if cores.has("pcsx2") {
        written.push("pcsx2");
        overrides.insert(
            "pcsx2_widescreen_hint",
            quoted(match ratio {
                AspectRatio::Standard => "disabled",
                AspectRatio::Wide16_9 => "enabled (16:9)",
                AspectRatio::Wide16_10 => "enabled (16:10)",
            }),
        );
    }

    // PPSSPP has no widescreen option at all (the nearest thing is
    // `ppsspp_cropto16x9`, which crops rather than widens the field of view),
    // and pcsx_rearmed has no `pcsx_rearmed_widescreen`. Snes9x's aspect option
    // is `snes9x_aspect` and is only read when RetroArch's aspect is "Core
    // provided", which apply_widescreen turns off for both wide ratios. None of
    // the three is written.

    info!(
        "[RETROARCH] Widescreen core options for {ratio:?} written for installed cores: {}",
        summarise(&written)
    );
}

/// Renders a written-cores list for the log, so "nothing applied" reads as
/// nothing applied instead of an empty tail.
fn summarise(written: &[&str]) -> String {
    if written.is_empty() {
        "none".to_string()
    } else {
        written.join(", ")
    }
}

/// Wraps a value in double quotes — the RetroArch core-options format.
fn quoted(value: &str) -> String {
    format!("\"{value}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every core Drop writes options for, as they appear in `cores/`.
    fn all_cores() -> InstalledCores {
        InstalledCores::from_names([
            "dolphin_libretro.so",
            "mupen64plus_next_libretro.so",
            "parallel_n64_libretro.so",
            "pcsx_rearmed_libretro.so",
            "beetle_psx_hw_libretro.so",
            "swanstation_libretro.so",
            "ppsspp_libretro.so",
            "mgba_libretro.so",
            "snes9x_libretro.so",
            "pcsx2_libretro.so",
        ])
    }

    const PRESETS: [QualityPreset; 4] = [
        QualityPreset::Low,
        QualityPreset::Medium,
        QualityPreset::High,
        QualityPreset::Ultra,
    ];

    const RATIOS: [AspectRatio; 3] =
        [AspectRatio::Standard, AspectRatio::Wide16_9, AspectRatio::Wide16_10];

    fn quality_map(quality: &QualityPreset, cores: &InstalledCores) -> HashMap<&'static str, String> {
        let mut map = HashMap::new();
        apply_core_quality_options(&mut map, quality, cores);
        map
    }

    fn widescreen_map(ratio: &AspectRatio, cores: &InstalledCores) -> HashMap<&'static str, String> {
        let mut map = HashMap::new();
        apply_core_widescreen_options(&mut map, ratio, cores);
        map
    }

    // ── Value lists taken from each core's own option table ───────────────
    //
    // A wrong value is invisible at runtime: RetroArch keeps the key and
    // silently substitutes the core default. These arrays are the only place
    // that failure mode is caught, so keep them in sync with the source they
    // name and nothing else.
    //
    // Most are the core's full list. MUPEN_169 and PCSX2_UPSCALE are subsets
    // covering the entries Drop actually writes — a subset can only be
    // over-strict (a legal value it omits fails the test), never let an
    // illegal value pass, so trimming them is safe.

    /// mupen64plus-libretro-nx `libretro/libretro_core_options.h`.
    const MUPEN_43: [&str; 13] = [
        "320x240", "640x480", "960x720", "1280x960", "1440x1080", "1600x1200", "1920x1440",
        "2240x1680", "2560x1920", "2880x2160", "3200x2400", "3520x2640", "3840x2880",
    ];
    const MUPEN_169: [&str; 16] = [
        "640x360", "960x540", "1280x720", "1706x720", "1366x768", "1920x810", "1920x1080",
        "2560x1080", "2560x1440", "3414x1440", "3840x2160", "4096x2160", "5120x2160", "7680x3240",
        "7680x4320", "10240x4320",
    ];
    const MUPEN_ASPECT: [&str; 3] = ["4:3", "16:9", "16:9 adjusted"];
    const MUPEN_TXFILTER: [&str; 7] = [
        "None", "Smooth filtering 1", "Smooth filtering 2", "Smooth filtering 3",
        "Smooth filtering 4", "Sharp filtering 1", "Sharp filtering 2",
    ];
    const MUPEN_TXENHANCE: [&str; 14] = [
        "None", "As Is", "X2", "X2SAI", "HQ2X", "HQ2XS", "LQ2X", "LQ2XS", "HQ4X", "2xBRZ", "3xBRZ",
        "4xBRZ", "5xBRZ", "6xBRZ",
    ];

    /// parallel-n64 `libretro/libretro_core_options.h`.
    const PARALLEL_SCREENSIZE: [&str; 10] = [
        "320x240", "640x480", "960x720", "1280x960", "1440x1080", "1600x1200", "1920x1440",
        "2240x1680", "2880x2160", "5760x4320",
    ];
    const PARALLEL_ASPECT_HINT: [&str; 2] = ["normal", "widescreen"];

    /// beetle-psx-libretro `libretro_core_options.h`.
    const BEETLE_RES: [&str; 5] = ["1x(native)", "2x", "4x", "8x", "16x"];
    const BEETLE_PGXP: [&str; 3] = ["disabled", "memory only", "memory + CPU"];
    const BEETLE_DITHER: [&str; 3] = ["1x(native)", "internal resolution", "disabled"];
    const BEETLE_FILTER: [&str; 6] = ["nearest", "SABR", "xBR", "bilinear", "3-point", "JINC2"];
    const BEETLE_WS_RATIO: [&str; 7] = ["16:9", "16:10", "18:9", "19:9", "20:9", "21:9", "32:9"];

    /// ppsspp `libretro/libretro_core_options.h`.
    const PPSSPP_RES: [&str; 10] = [
        "480x272", "960x544", "1440x816", "1920x1088", "2400x1360", "2880x1632", "3360x1904",
        "3840x2176", "4320x2448", "4800x2720",
    ];
    const PPSSPP_SCALING_TYPE: [&str; 4] = ["xbrz", "hybrid", "bicubic", "hybrid_bicubic"];
    const PPSSPP_SCALING_LEVEL: [&str; 5] = ["disabled", "2x", "3x", "4x", "5x"];
    const PPSSPP_TEXFILTER: [&str; 4] = ["Auto", "Nearest", "Linear", "Auto max quality"];

    /// mgba `src/platform/libretro/libretro_core_options.h`.
    const MGBA_COLOR: [&str; 4] = ["OFF", "GBA", "GBC", "Auto"];

    /// snes9x `libretro/libretro_core_options.h`.
    const SNES9X_HIRES_BLEND: [&str; 3] = ["disabled", "merge", "blur"];

    /// swanstation `src/libretro/libretro_core_options.h` (1x..16x, 1..16).
    const SWANSTATION_WS: [&str; 2] = ["true", "false"];

    /// libretro/ps2 `libretro/libretro_core_options.h`.
    const PCSX2_DEINTERLACE: [&str; 10] = [
        "Automatic", "Off", "Weave TFF", "Weave BFF", "Bob TFF", "Bob BFF", "Blend TFF",
        "Blend BFF", "Adaptive TFF", "Adaptive BFF",
    ];
    const PCSX2_UPSCALE: [&str; 6] = [
        "1x Native (PS2)", "2x Native (~720p)", "3x Native (~1080p)", "4x Native (~1440p/2K)",
        "5x Native (~1800p/3K)", "6x Native (~2160p/4K)",
    ];
    const PCSX2_ANISO: [&str; 5] = ["disabled", "2x", "4x", "8x", "16x"];
    const PCSX2_DITHER: [&str; 4] = ["disabled", "Scaled", "Unscaled", "Force 32bit"];
    const PCSX2_TEXFILTER: [&str; 4] = [
        "Nearest", "Bilinear (Forced)", "Bilinear (PS2)", "Bilinear (Forced excluding sprite)",
    ];
    const PCSX2_BLEND: [&str; 6] = ["Minimum", "Basic", "Medium", "High", "Full", "Maximum"];
    const PCSX2_WS_HINT: [&str; 5] = [
        "disabled", "enabled (16:9)", "enabled (16:10)", "enabled (21:9)", "enabled (32:9)",
    ];

    /// DolphinLibretro `Common/Options.cpp`.
    const DOLPHIN_ASPECT: [&str; 7] = ["0", "1", "2", "3", "4", "5", "6"];
    const DOLPHIN_EFB_SCALE: [&str; 6] = ["1", "2", "3", "4", "5", "6"];
    const DOLPHIN_AA: [&str; 7] = ["0", "1", "2", "3", "4", "5", "6"];
    const DOLPHIN_ANISO: [&str; 5] = ["0", "1", "2", "3", "4"];
    const DOLPHIN_TEXFILTER_MODE: [&str; 3] = ["0", "1", "2"];
    const ENABLED_DISABLED: [&str; 2] = ["disabled", "enabled"];

    /// Asserts the (quoted) value written for `key` is a member of the core's
    /// own value list.
    #[track_caller]
    fn assert_legal(map: &HashMap<&str, String>, key: &str, legal: &[&str]) {
        let written = map.get(key).unwrap_or_else(|| panic!("{key} was not written"));
        let bare = written
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or_else(|| panic!("{key} = {written} is not quoted"));
        assert!(
            legal.contains(&bare),
            "{key} = {bare:?} is not in the core's value list {legal:?}"
        );
    }

    #[test]
    fn every_quality_value_is_in_its_core_option_table() {
        let cores = all_cores();
        for quality in &PRESETS {
            let map = quality_map(quality, &cores);

            assert_legal(&map, "dolphin_efb_scale", &DOLPHIN_EFB_SCALE);
            assert_legal(&map, "dolphin_anti_aliasing", &DOLPHIN_AA);
            assert_legal(&map, "dolphin_efb_access_enable", &ENABLED_DISABLED);
            assert_legal(&map, "dolphin_max_anisotropy", &DOLPHIN_ANISO);
            assert_legal(&map, "dolphin_force_texture_filtering_mode", &DOLPHIN_TEXFILTER_MODE);
            assert_legal(&map, "dolphin_progressive_scan", &ENABLED_DISABLED);
            assert_legal(&map, "dolphin_renderer", &["Hardware"]);

            assert_legal(&map, "mupen64plus-43screensize", &MUPEN_43);
            assert_legal(&map, "mupen64plus-169screensize", &MUPEN_169);
            assert_legal(&map, "mupen64plus-txFilterMode", &MUPEN_TXFILTER);
            assert_legal(&map, "mupen64plus-txEnhancementMode", &MUPEN_TXENHANCE);

            assert_legal(&map, "parallel-n64-screensize", &PARALLEL_SCREENSIZE);

            assert_legal(&map, "pcsx_rearmed_neon_enhancement_enable", &ENABLED_DISABLED);
            assert_legal(&map, "pcsx_rearmed_neon_enhancement_no_main", &ENABLED_DISABLED);

            assert_legal(&map, "beetle_psx_hw_internal_resolution", &BEETLE_RES);
            assert_legal(&map, "beetle_psx_hw_dither_mode", &BEETLE_DITHER);
            assert_legal(&map, "beetle_psx_hw_filter", &BEETLE_FILTER);
            assert_legal(&map, "beetle_psx_hw_pgxp_mode", &BEETLE_PGXP);
            assert_legal(&map, "beetle_psx_hw_pgxp_texture", &ENABLED_DISABLED);

            assert_legal(&map, "swanstation_GPU_ResolutionScale", &["1", "2", "4", "8"]);

            assert_legal(&map, "ppsspp_internal_resolution", &PPSSPP_RES);
            assert_legal(&map, "ppsspp_texture_filtering", &PPSSPP_TEXFILTER);

            assert_legal(&map, "mgba_color_correction", &MGBA_COLOR);
            assert_legal(&map, "snes9x_hires_blend", &SNES9X_HIRES_BLEND);

            assert_legal(&map, "pcsx2_upscale_multiplier", &PCSX2_UPSCALE);
            assert_legal(&map, "pcsx2_anisotropic_filtering", &PCSX2_ANISO);
            assert_legal(&map, "pcsx2_dithering", &PCSX2_DITHER);
            assert_legal(&map, "pcsx2_texture_filtering", &PCSX2_TEXFILTER);
            assert_legal(&map, "pcsx2_blending_accuracy", &PCSX2_BLEND);
            assert_legal(&map, "pcsx2_deinterlace_mode", &PCSX2_DEINTERLACE);
            assert_legal(&map, "pcsx2_renderer", &["Auto"]);
        }
    }

    #[test]
    fn ultra_ppsspp_texture_scaling_is_legal_and_only_on_ultra() {
        let cores = all_cores();
        let ultra = quality_map(&QualityPreset::Ultra, &cores);
        assert_legal(&ultra, "ppsspp_texture_scaling_type", &PPSSPP_SCALING_TYPE);
        assert_legal(&ultra, "ppsspp_texture_scaling_level", &PPSSPP_SCALING_LEVEL);

        let high = quality_map(&QualityPreset::High, &cores);
        assert!(!high.contains_key("ppsspp_texture_scaling_type"));
        assert!(!high.contains_key("ppsspp_texture_scaling_level"));
    }

    #[test]
    fn every_widescreen_value_is_in_its_core_option_table() {
        let cores = all_cores();
        for ratio in &RATIOS {
            let map = widescreen_map(ratio, &cores);

            assert_legal(&map, "dolphin_widescreen_hack", &ENABLED_DISABLED);
            assert_legal(&map, "dolphin_aspect_ratio", &DOLPHIN_ASPECT);
            assert_legal(&map, "dolphin_widescreen", &ENABLED_DISABLED);
            assert_legal(&map, "mupen64plus-aspect", &MUPEN_ASPECT);
            assert_legal(&map, "parallel-n64-aspectratiohint", &PARALLEL_ASPECT_HINT);
            assert_legal(&map, "beetle_psx_hw_widescreen_hack", &ENABLED_DISABLED);
            assert_legal(&map, "swanstation_GPU_WidescreenHack", &SWANSTATION_WS);
            assert_legal(&map, "pcsx2_widescreen_hint", &PCSX2_WS_HINT);
        }
    }

    #[test]
    fn beetle_widescreen_ratio_is_only_written_when_wide() {
        let cores = all_cores();
        // The option's list has no 4:3 member, so on Standard the hack is
        // switched off and the ratio key must be left untouched.
        let standard = widescreen_map(&AspectRatio::Standard, &cores);
        assert!(!standard.contains_key("beetle_psx_hw_widescreen_hack_aspect_ratio"));

        for ratio in [AspectRatio::Wide16_9, AspectRatio::Wide16_10] {
            let map = widescreen_map(&ratio, &cores);
            assert_legal(&map, "beetle_psx_hw_widescreen_hack_aspect_ratio", &BEETLE_WS_RATIO);
        }
    }

    #[test]
    fn dolphin_aspect_is_auto_on_standard_and_force_wide_on_widescreen() {
        let cores = all_cores();
        let standard = widescreen_map(&AspectRatio::Standard, &cores);
        assert_eq!(standard["dolphin_aspect_ratio"], "\"0\""); // Auto
        assert_eq!(standard["dolphin_widescreen"], "\"disabled\"");

        let wide = widescreen_map(&AspectRatio::Wide16_9, &cores);
        assert_eq!(wide["dolphin_aspect_ratio"], "\"1\""); // Force Wide
        assert_eq!(wide["dolphin_widescreen"], "\"enabled\"");
    }

    #[test]
    fn n64_aspect_uses_the_adjusted_widescreen_hack() {
        let cores = all_cores();
        let wide = widescreen_map(&AspectRatio::Wide16_9, &cores);
        // Plain "16:9" only stretches the 4:3 image; "16:9 adjusted" widens
        // the actual projection.
        assert_eq!(wide["mupen64plus-aspect"], "\"16:9 adjusted\"");
    }

    #[test]
    fn keys_the_cores_reject_are_never_written() {
        // Each of these was written at some point and silently dropped: wrong
        // key entirely, or a DuckStation-style dotted separator.
        const DEAD_KEYS: [&str; 8] = [
            "mupen64plus-Resolution",
            "ppsspp_widescreen_hack",
            "pcsx2_renderer_backend",
            "pcsx_rearmed_widescreen",
            "snes9x_aspect_ratio",
            "swanstation_GPU.ResolutionScale",
            "swanstation_GPU.WidescreenHack",
            "duckstation_GPU.ResolutionScale",
        ];
        let cores = all_cores();
        let mut map = HashMap::new();
        for quality in &PRESETS {
            apply_core_quality_options(&mut map, quality, &cores);
        }
        for ratio in &RATIOS {
            apply_core_widescreen_options(&mut map, ratio, &cores);
        }
        for dead in DEAD_KEYS {
            assert!(!map.contains_key(dead), "{dead} is still being written");
        }
        // DuckStation is not shipped and its option table could not be checked,
        // so nothing under its prefix may be written at all.
        assert!(map.keys().all(|k| !k.starts_with("duckstation")));
    }

    #[test]
    fn nothing_is_written_when_no_cores_are_installed() {
        let none = InstalledCores::from_names(Vec::<&str>::new());
        assert!(quality_map(&QualityPreset::Ultra, &none).is_empty());
        assert!(widescreen_map(&AspectRatio::Wide16_9, &none).is_empty());
    }

    #[test]
    fn only_the_installed_cores_get_keys() {
        let cores = InstalledCores::from_names(["dolphin_libretro.dll"]);
        let map = quality_map(&QualityPreset::High, &cores);
        assert!(!map.is_empty());
        assert!(
            map.keys().all(|k| k.starts_with("dolphin_")),
            "wrote non-Dolphin keys with only Dolphin installed: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn core_matching_is_by_filename_fragment() {
        // The shipped N64 core is mupen64plus_next; the option keys are still
        // the plain mupen64plus- ones, so the fragment has to match loosely.
        let cores = InstalledCores::from_names(["mupen64plus_next_libretro.so"]);
        assert!(cores.has("mupen64plus"));
        assert!(!cores.has("parallel_n64"));

        // Platform suffix and case must not matter.
        let win = InstalledCores::from_names(["PPSSPP_libretro.DLL"]);
        assert!(win.has("ppsspp"));

        // pcsx_rearmed must not be mistaken for pcsx2 or vice versa.
        let rearmed = InstalledCores::from_names(["pcsx_rearmed_libretro.so"]);
        assert!(rearmed.has("pcsx_rearmed"));
        assert!(!rearmed.has("pcsx2"));
        let ps2 = InstalledCores::from_names(["pcsx2_libretro.so"]);
        assert!(ps2.has("pcsx2"));
        assert!(!ps2.has("pcsx_rearmed"));
    }

    #[test]
    fn n64_resolution_pair_matches_the_selected_aspect_list() {
        // mupen64plus-aspect selects which screensize list the core reads, so
        // both lists have to carry a value for every preset — a 4:3 value in
        // the 16:9 list would be dropped the moment the user goes widescreen.
        for quality in &PRESETS {
            let res = resolution_targets(quality);
            assert!(MUPEN_43.contains(&res.n64_res_43));
            assert!(MUPEN_169.contains(&res.n64_res_169));
        }
    }
}

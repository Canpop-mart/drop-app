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
//! Core-option keys for *every* known core are written unconditionally so the
//! setting takes effect regardless of which core ends up running the game.

use database::models::data::{AspectRatio, QualityPreset};
use log::info;
use std::collections::HashMap;

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

// ── Core options (retroarch-core-options.cfg) ────────────────────────────

/// Applies per-core internal-resolution and image-quality options for the
/// quality preset to `retroarch-core-options.cfg`.
///
/// Every known core's keys are written so the setting applies whichever core
/// runs the game.
pub fn apply_core_quality_options(overrides: &mut HashMap<&str, String>, quality: &QualityPreset) {
    let (dolphin_efb, n64_res, beetle_psx_res, ppsspp_res) = match quality {
        QualityPreset::Low => ("1", "320x240", "1x(native)", "1"),
        QualityPreset::Medium => ("3", "640x480", "2x(native)", "2"),
        QualityPreset::High => ("5", "1280x960", "4x(native)", "4"),
        QualityPreset::Ultra => ("6", "1920x1440", "8x(native)", "8"),
    };

    // Dolphin (GameCube/Wii) — internal EFB scale (Ultra 6x = max).
    overrides.insert("dolphin_efb_scale", quoted(dolphin_efb));

    // Mupen64Plus-Next / parallel-n64 (N64) — render resolution.
    overrides.insert("mupen64plus-Resolution", quoted(n64_res));
    overrides.insert("parallel-n64-screensize", quoted(n64_res));

    // PCSX ReARMed (PS1) — NEON enhancement.
    overrides.insert(
        "pcsx_rearmed_neon_enhancement_enable",
        quoted(if matches!(quality, QualityPreset::Low) { "disabled" } else { "enabled" }),
    );
    overrides.insert("pcsx_rearmed_neon_enhancement_no_main", "\"disabled\"".into());

    // Beetle PSX HW (PS1 HW) — internal GPU resolution.
    overrides.insert("beetle_psx_hw_internal_resolution", quoted(beetle_psx_res));

    // SwanStation / DuckStation (PS1) — GPU resolution scale.
    let ps1_res_scale = match quality {
        QualityPreset::Low => "1",
        QualityPreset::Medium => "2",
        QualityPreset::High => "4",
        QualityPreset::Ultra => "8",
    };
    overrides.insert("swanstation_GPU.ResolutionScale", quoted(ps1_res_scale));
    overrides.insert("duckstation_GPU.ResolutionScale", quoted(ps1_res_scale));

    // PPSSPP (PSP) — internal resolution.
    overrides.insert("ppsspp_internal_resolution", quoted(ppsspp_res));

    // mGBA (GBA) — colour correction.
    overrides.insert(
        "mgba_color_correction",
        quoted(if matches!(quality, QualityPreset::Low) { "OFF" } else { "Game Boy Advance" }),
    );

    // Snes9x — hi-res blending.
    overrides.insert(
        "snes9x_hires_blend",
        quoted(if matches!(quality, QualityPreset::Low) { "disabled" } else { "merge" }),
    );

    // Beetle PSX HW — dithering / filtering / PGXP.
    let (psx_dither, psx_filter, psx_pgxp) = match quality {
        QualityPreset::Low => ("1x(native)", "nearest", "disabled"),
        QualityPreset::Medium => ("1x(native)", "nearest", "enabled"),
        QualityPreset::High | QualityPreset::Ultra => ("disabled", "bilinear", "enabled"),
    };
    overrides.insert("beetle_psx_hw_dither_mode", quoted(psx_dither));
    overrides.insert("beetle_psx_hw_filter", quoted(psx_filter));
    overrides.insert("beetle_psx_hw_pgxp_mode", quoted(psx_pgxp));
    // Ultra: PGXP perspective-correct texturing kills PS1 texture warping.
    overrides.insert(
        "beetle_psx_hw_pgxp_texture",
        quoted(if matches!(quality, QualityPreset::Ultra) { "enabled" } else { "disabled" }),
    );

    // PCSX2 — Adaptive deinterlacer avoids ghosted UI text on 480i games.
    overrides.insert("pcsx2_deinterlace_mode", "\"Adaptive\"".into());

    // Mupen64Plus-Next — texture filtering + aspect.
    let (n64_txfilter, n64_aspect) = match quality {
        QualityPreset::Low | QualityPreset::Medium => ("None", "4:3"),
        QualityPreset::High | QualityPreset::Ultra => ("6xBRZ", "16:9 adjusted"),
    };
    overrides.insert("mupen64plus-txFilterMode", quoted(n64_txfilter));
    overrides.insert("mupen64plus-aspect", quoted(n64_aspect));

    // Dolphin — anti-aliasing + EFB access.
    let (dolphin_aa, dolphin_efb_copy) = match quality {
        QualityPreset::Low => ("None", "disabled"),
        QualityPreset::Medium => ("2x MSAA", "enabled"),
        QualityPreset::High => ("4x MSAA", "enabled"),
        QualityPreset::Ultra => ("8x MSAA", "enabled"),
    };
    overrides.insert("dolphin_anti_aliasing", quoted(dolphin_aa));
    overrides.insert("dolphin_efb_access_enable", quoted(dolphin_efb_copy));

    // Additional Dolphin (GameCube / Wii) image-quality knobs — Wii in
    // particular needs more than just EFB scale to look right at modern
    // resolutions:
    //   * anisotropic filtering — keeps oblique-angle textures sharp at
    //     higher internal resolutions; scales with the preset.
    //   * force_texture_filtering — smooths nearest-neighbour textures
    //     (skipped on Low to preserve original-look perf).
    //   * progressive scan — Wii outputs 480p; forcing it on avoids
    //     interlace artifacts that PAL/JP titles ship with by default.
    let dolphin_aniso = match quality {
        QualityPreset::Low => "1x",
        QualityPreset::Medium => "2x",
        QualityPreset::High => "4x",
        QualityPreset::Ultra => "16x",
    };
    overrides.insert("dolphin_anisotropic_filtering", quoted(dolphin_aniso));
    overrides.insert(
        "dolphin_force_texture_filtering",
        quoted(if matches!(quality, QualityPreset::Low) {
            "disabled"
        } else {
            "enabled"
        }),
    );
    overrides.insert("dolphin_progressive_scan", "\"enabled\"".into());

    // PPSSPP — texture filtering + scaling.
    let ppsspp_texfilter = match quality {
        QualityPreset::Low | QualityPreset::Medium => "Auto",
        QualityPreset::High | QualityPreset::Ultra => "Linear",
    };
    overrides.insert("ppsspp_texture_filtering", quoted(ppsspp_texfilter));
    if matches!(quality, QualityPreset::Ultra) {
        overrides.insert("ppsspp_texture_scaling_type", "\"xBRZ\"".into());
        overrides.insert("ppsspp_texture_scaling_level", "\"3\"".into());
    }

    // PCSX2 / LRPS2 (PS2) — internal-resolution upscale + image quality.
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

    // PCSX2 renderer — keep hardware rendering; "Auto" follows the RA driver.
    overrides.insert("pcsx2_renderer", "\"Auto\"".into());
    overrides.insert("pcsx2_renderer_backend", "\"Auto\"".into());
    info!("[RETROARCH] PCSX2 core options: renderer=Auto, backend=Auto, upscale={pcsx2_res}");
}

/// Applies per-core widescreen-hack options to `retroarch-core-options.cfg`.
///
/// Native widescreen hacks are enabled for both 16:9 and 16:10; the hacks
/// render at native wide resolution and [`apply_widescreen`] sets the final
/// display ratio.
pub fn apply_core_widescreen_options(overrides: &mut HashMap<&str, String>, ratio: &AspectRatio) {
    let enabled = !matches!(ratio, AspectRatio::Standard);
    let enabled_disabled = if enabled { "enabled" } else { "disabled" };
    let on_off = if enabled { "ON" } else { "OFF" };
    let ratio_str = match ratio {
        AspectRatio::Standard => "4:3",
        AspectRatio::Wide16_9 => "16:9",
        AspectRatio::Wide16_10 => "16:10",
    };
    // N64 cores only support 16:9 or 4:3.
    let n64_aspect = if enabled { "16:9" } else { "4:3" };
    let bool_str = if enabled { "true" } else { "false" };

    overrides.insert("dolphin_widescreen_hack", quoted(enabled_disabled));
    overrides.insert("mupen64plus-aspect", quoted(n64_aspect));
    overrides.insert("parallel-n64-aspect", quoted(n64_aspect));
    overrides.insert("ppsspp_widescreen_hack", quoted(on_off));
    overrides.insert("beetle_psx_hw_widescreen_hack", quoted(enabled_disabled));
    overrides.insert("beetle_psx_hw_widescreen_hack_aspect_ratio", quoted(ratio_str));
    overrides.insert("swanstation_GPU.WidescreenHack", quoted(bool_str));
    overrides.insert("duckstation_GPU.WidescreenHack", quoted(bool_str));
    overrides.insert("pcsx_rearmed_widescreen", quoted(enabled_disabled));
    overrides.insert("pcsx2_widescreen_hint", quoted(enabled_disabled));
    overrides.insert("snes9x_aspect_ratio", quoted(n64_aspect));
}

/// Wraps a value in double quotes — the RetroArch core-options format.
fn quoted(value: &str) -> String {
    format!("\"{value}\"")
}

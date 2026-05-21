//! CRT shader management.
//!
//! When the user enables the CRT shader for a game, Drop must make RetroArch
//! actually auto-apply one. The `video_shader` config key alone is *not*
//! enough — RetroArch treats it as a "last used" value. The reliable path is
//! an **auto-shader preset** at `<shader_dir>/presets/global.slangp`, which
//! RetroArch loads on every content launch.
//!
//! Shader source priority:
//! 1. A system CRT shader on disk (crt-easymode / crt-lottes / …),
//! 2. otherwise the bundled `drop-crt` shader, written to disk every launch
//!    so Drop never depends on RetroArch shipping shader files.
//!
//! CRT shaders split into two compatibility classes — low-res-only (great on
//! 2D consoles, black screen on upscaled 3D) and resolution-tolerant. The
//! caller passes `prefer_high_res_capable` for ROMs that resolve to high-res
//! 3D cores so a resolution-tolerant shader is chosen.

use log::{info, warn};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::cfg::path_to_cfg;
use super::discovery::find_appimage_config_dir;

/// Enables the CRT shader: writes the bundled shader, picks the best
/// available preset, and wires up reliable auto-apply. Returns the chosen
/// shader path (for the frontend), or `None` if nothing could be set up.
pub fn apply_crt_shader(
    overrides: &mut HashMap<&str, String>,
    emu_root: &Path,
    prefer_high_res_capable: bool,
) -> Option<String> {
    overrides.insert("video_shader_enable", "true".into());

    let shader_dir = emu_root.join("shaders");
    overrides.insert("auto_shaders_enable", "true".into());
    overrides.insert("video_shader_dir", path_to_cfg(&shader_dir));

    // Clear stale per-core/per-content presets that would outrank our global.
    remove_auto_shader_presets(emu_root);

    // Always write the bundled shader so there is a guaranteed fallback.
    let bundled_path = write_bundled_crt_shader(emu_root);

    let chosen_shader = find_best_crt_shader(emu_root, prefer_high_res_capable)
        .or_else(|| {
            #[cfg(target_os = "linux")]
            {
                if let Some(appimage_path) = super::discovery::find_appimage_binary(emu_root) {
                    info!("[RETROARCH] No system shaders found — extracting from AppImage");
                    extract_appimage_shaders(emu_root, &appimage_path);
                    return find_best_crt_shader(emu_root, prefer_high_res_capable);
                }
            }
            None
        })
        .or(bundled_path);

    let preset_path = match chosen_shader {
        Some(p) => p,
        None => {
            warn!("[RETROARCH] CRT shader enabled but no shader available");
            return None;
        }
    };

    info!("[RETROARCH] Selected CRT shader: {}", preset_path.display());

    let raw = preset_path.to_string_lossy().into_owned();
    overrides.insert("video_shader", path_to_cfg(&preset_path));
    write_auto_shader_preset(emu_root, &preset_path);

    Some(raw)
}

/// Disables shaders in `overrides` and removes stale auto-shader presets so
/// RetroArch doesn't load a shader the user turned off.
pub fn disable_crt_shader(overrides: &mut HashMap<&str, String>, emu_root: &Path) {
    overrides.insert("video_shader_enable", "false".into());
    overrides.insert("video_shader", "\"\"".into());
    overrides.insert("auto_shaders_enable", "false".into());
    remove_auto_shader_presets(emu_root);
}

/// Removes **all** auto-shader presets (global + per-core + per-content) so
/// only Drop's choice (a CRT shader, or nothing) is ever loaded.
///
/// RetroArch's auto-shader priority is per-game > per-core > global; stale
/// per-core/per-game presets from earlier sessions outrank Drop's global one
/// and can carry display overrides that block widescreen.
fn remove_auto_shader_presets(emu_root: &Path) {
    let presets_dir = emu_root.join("shaders").join("presets");
    if !presets_dir.is_dir() {
        return;
    }

    for name in &["global.slangp", "global.glslp"] {
        let p = presets_dir.join(name);
        if p.exists() {
            match fs::remove_file(&p) {
                Ok(_) => info!("[RETROARCH] Removed auto-shader preset: {}", p.display()),
                Err(e) => warn!("[RETROARCH] Failed to remove auto-shader preset {}: {e}", p.display()),
            }
        }
    }

    if let Ok(entries) = fs::read_dir(&presets_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(files) = fs::read_dir(&path) {
                for file in files.flatten() {
                    let fp = file.path();
                    if matches!(fp.extension().and_then(|e| e.to_str()), Some("slangp" | "glslp")) {
                        match fs::remove_file(&fp) {
                            Ok(_) => info!("[RETROARCH] Removed stale shader preset: {}", fp.display()),
                            Err(e) => warn!("[RETROARCH] Failed to remove stale shader preset {}: {e}", fp.display()),
                        }
                    }
                }
            }
        }
    }
}

/// Writes the global auto-shader preset referencing `shader_preset_path`.
///
/// Written to `<emu_root>/shaders/presets/global.{slangp,glslp}`. For the
/// bundled shader, self-contained absolute-path presets are written (no
/// `#reference` needed); for system shaders the modern `#reference` directive
/// is used with the bundled shader as the other-format fallback.
fn write_auto_shader_preset(emu_root: &Path, shader_preset_path: &Path) {
    let presets_dir = emu_root.join("shaders").join("presets");
    if let Err(e) = fs::create_dir_all(&presets_dir) {
        warn!("[RETROARCH] Failed to create auto-shader presets dir: {e}");
        return;
    }

    let is_slang = shader_preset_path.extension().and_then(|e| e.to_str()) == Some("slangp");
    let preset_path_str = shader_preset_path.to_string_lossy().replace('\\', "/");
    let reference_content = format!("#reference \"{preset_path_str}\"\n");

    let bundled_dir = emu_root.join("shaders").join("drop-crt");
    let slang_abs = bundled_dir.join("drop-crt.slang").to_string_lossy().replace('\\', "/");
    let glsl_abs = bundled_dir.join("drop-crt.glsl").to_string_lossy().replace('\\', "/");

    let slangp_absolute = format!(
        "shaders = \"1\"\n\
         shader0 = \"{slang_abs}\"\n\
         filter_linear0 = \"true\"\n\
         wrap_mode0 = \"clamp_to_border\"\n\
         mipmap_input0 = \"false\"\n\
         alias0 = \"\"\n\
         float_framebuffer0 = \"false\"\n\
         srgb_framebuffer0 = \"false\"\n\
         scale_type0 = \"viewport\"\n\
         scale0 = \"1.000000\"\n"
    );
    let glslp_absolute = format!(
        "shaders = \"1\"\n\
         shader0 = \"{glsl_abs}\"\n\
         filter_linear0 = \"true\"\n\
         wrap_mode0 = \"clamp_to_border\"\n\
         scale_type0 = \"viewport\"\n\
         scale0 = \"1.000000\"\n"
    );

    let is_bundled = shader_preset_path.starts_with(&bundled_dir);

    if is_bundled {
        write_preset(&presets_dir.join("global.slangp"), &slangp_absolute, "bundled slangp");
        write_preset(&presets_dir.join("global.glslp"), &glslp_absolute, "bundled glslp");
    } else {
        let target_name = if is_slang { "global.slangp" } else { "global.glslp" };
        write_preset(&presets_dir.join(target_name), &reference_content, "#reference");
        // Write the bundled shader as the other format for driver compat.
        if is_slang {
            let _ = fs::write(presets_dir.join("global.glslp"), &glslp_absolute);
        } else {
            let _ = fs::write(presets_dir.join("global.slangp"), &slangp_absolute);
        }
    }
}

fn write_preset(path: &Path, content: &str, kind: &str) {
    match fs::write(path, content) {
        Ok(_) => info!("[RETROARCH] Wrote auto-shader preset ({kind}): {}", path.display()),
        Err(e) => warn!("[RETROARCH] Failed to write auto-shader preset ({kind}): {e}"),
    }
}

/// Searches all known locations for a high-quality system CRT shader preset.
///
/// For low-res 2D consoles `crt-easymode` wins (strong scanlines at native
/// res). For high-res 3D cores it produces black output, so resolution-
/// tolerant math-based shaders (`crt-lottes`, `crt-royale-fast`) are preferred.
fn find_best_crt_shader(emu_root: &Path, prefer_high_res_capable: bool) -> Option<PathBuf> {
    let preferred: &[&str] = if prefer_high_res_capable {
        &[
            "crt-lottes.slangp",
            "crt-royale-fast.slangp",
            "crt-geom-flat.slangp",
            "crt-geom.slangp",
            "crt-royale.slangp",
            "crt-lottes.glslp",
            "crt-royale-fast.glslp",
            "crt-geom-flat.glslp",
            "crt-geom.glslp",
            "crt-royale.glslp",
        ]
    } else {
        &[
            "crt-easymode.slangp",
            "crt-royale.slangp",
            "crt-lottes.slangp",
            "crt-easymode.glslp",
            "crt-royale.glslp",
            "crt-lottes.glslp",
        ]
    };

    let mut shader_dirs: Vec<PathBuf> = vec![
        emu_root.join("shaders").join("shaders_slang").join("crt"),
        emu_root.join("shaders").join("shaders_glsl").join("crt"),
        emu_root.join("shaders_slang").join("crt"),
        emu_root.join("shaders_glsl").join("crt"),
    ];
    if let Some(ai_cfg_dir) = find_appimage_config_dir(emu_root) {
        shader_dirs.push(ai_cfg_dir.join("shaders/shaders_slang/crt"));
        shader_dirs.push(ai_cfg_dir.join("shaders/shaders_glsl/crt"));
    }
    let squashfs_root = emu_root.join("squashfs-root");
    shader_dirs.push(squashfs_root.join("usr/share/libretro/shaders/shaders_slang/crt"));
    shader_dirs.push(squashfs_root.join("usr/share/retroarch/shaders/shaders_slang/crt"));

    find_crt_shader_in_dirs(&shader_dirs, preferred)
}

/// Returns the first matching CRT shader preset across `dirs`, preferring the
/// names in `preferred` before falling back to any `.slangp`/`.glslp`.
fn find_crt_shader_in_dirs(dirs: &[PathBuf], preferred: &[&str]) -> Option<PathBuf> {
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        for preset in preferred {
            let path = dir.join(preset);
            if path.is_file() {
                return Some(path);
            }
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().is_some_and(|e| e == "slangp" || e == "glslp") {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Extracts shader files from the RetroArch AppImage via `--appimage-extract`.
#[cfg(target_os = "linux")]
fn extract_appimage_shaders(emu_root: &Path, appimage_path: &Path) {
    use std::process::Command;

    let try_extract = |glob: &str| {
        Command::new(appimage_path)
            .arg("--appimage-extract")
            .arg(glob)
            .current_dir(emu_root)
            .output()
    };

    match try_extract("usr/share/libretro/shaders/shaders_slang/crt/*") {
        Ok(output) if output.status.success() => {
            info!("[RETROARCH] Extracted CRT shaders from AppImage (slang)");
        }
        _ => match try_extract("usr/share/retroarch/shaders/shaders_slang/crt/*") {
            Ok(o) if o.status.success() => {
                info!("[RETROARCH] Extracted CRT shaders from AppImage (alt path)");
            }
            _ => warn!("[RETROARCH] Failed to extract shaders from AppImage"),
        },
    }
}

// ── Bundled CRT shader ───────────────────────────────────────────────────
//
// A self-contained CRT shader embedded in the Drop binary. Written to disk
// on every launch so Drop never depends on RetroArch shipping shader files.

/// Slang CRT shader source (Vulkan / GLCore drivers).
const DROP_CRT_SLANG: &str = r#"#version 450

// Drop CRT — scanline + shadow mask shader
// Written for the Vulkan/GLCore slang pipeline.

layout(push_constant) uniform Push
{
    vec4 SourceSize;
    vec4 OriginalSize;
    vec4 OutputSize;
    uint FrameCount;
} params;

layout(std140, set = 0, binding = 0) uniform UBO
{
    mat4 MVP;
} global;

#pragma stage vertex
layout(location = 0) in vec4 Position;
layout(location = 1) in vec2 TexCoord;
layout(location = 0) out vec2 vTexCoord;

void main()
{
    gl_Position = global.MVP * Position;
    vTexCoord = TexCoord;
}

#pragma stage fragment
layout(location = 0) in vec2 vTexCoord;
layout(location = 0) out vec4 FragColor;
layout(set = 0, binding = 2) uniform sampler2D Source;

void main()
{
    vec3 col = texture(Source, vTexCoord).rgb;

    // ── Scanlines ───────────────────────────────────────────────────────
    // One dark band per source scanline, intensity modulated by output position.
    float scanY = vTexCoord.y * params.SourceSize.y * 2.0 * 3.14159265;
    float scanline = 0.5 * sin(scanY) + 0.5;           // 0..1 wave
    scanline = mix(0.70, 1.0, scanline);                // 30% max darkening

    // ── Shadow mask (RGB triad) ─────────────────────────────────────────
    // Subtle per-subpixel tint that becomes visible at higher output resolutions.
    float maskX = mod(floor(vTexCoord.x * params.OutputSize.x), 3.0);
    vec3 mask = vec3(1.0, 0.85, 0.85);
    if (maskX > 0.5 && maskX < 1.5) mask = vec3(0.85, 1.0, 0.85);
    else if (maskX > 1.5)            mask = vec3(0.85, 0.85, 1.0);

    col *= scanline;
    col *= mask;

    // Brightness bump to compensate for darkening
    col *= 1.30;

    FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

/// Slang preset file referencing the shader source.
const DROP_CRT_SLANGP: &str = r#"shaders = "1"

shader0 = "drop-crt.slang"
filter_linear0 = "true"
wrap_mode0 = "clamp_to_border"
mipmap_input0 = "false"
alias0 = ""
float_framebuffer0 = "false"
srgb_framebuffer0 = "false"
scale_type0 = "viewport"
scale0 = "1.000000"
"#;

/// GLSL CRT shader source (legacy GL driver fallback).
const DROP_CRT_GLSL: &str = r#"// Drop CRT — GLSL version for legacy GL driver

#if defined(VERTEX)

attribute vec4 VertexCoord;
attribute vec2 TexCoord;
varying vec2 vTexCoord;

uniform mat4 MVPMatrix;

void main()
{
    gl_Position = MVPMatrix * VertexCoord;
    vTexCoord = TexCoord;
}

#elif defined(FRAGMENT)

#ifdef GL_ES
precision mediump float;
#endif

uniform sampler2D Texture;
uniform vec2 InputSize;
uniform vec2 OutputSize;

varying vec2 vTexCoord;

void main()
{
    vec3 col = texture2D(Texture, vTexCoord).rgb;

    // Scanlines
    float scanY = vTexCoord.y * InputSize.y * 2.0 * 3.14159265;
    float scanline = 0.5 * sin(scanY) + 0.5;
    scanline = mix(0.70, 1.0, scanline);

    // Shadow mask
    float maskX = mod(floor(vTexCoord.x * OutputSize.x), 3.0);
    vec3 mask = vec3(1.0, 0.85, 0.85);
    if (maskX > 0.5 && maskX < 1.5) mask = vec3(0.85, 1.0, 0.85);
    else if (maskX > 1.5)            mask = vec3(0.85, 0.85, 1.0);

    col *= scanline;
    col *= mask;
    col *= 1.30;

    gl_FragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}

#endif
"#;

/// GLSL preset file.
const DROP_CRT_GLSLP: &str = r#"shaders = "1"

shader0 = "drop-crt.glsl"
filter_linear0 = "true"
wrap_mode0 = "clamp_to_border"
scale_type0 = "viewport"
scale0 = "1.000000"
"#;

/// Writes the bundled CRT shader files to `<emu_root>/shaders/drop-crt/`.
/// Returns the `.slangp` preset path (preferred) or the `.glslp` fallback.
fn write_bundled_crt_shader(emu_root: &Path) -> Option<PathBuf> {
    let shader_dir = emu_root.join("shaders").join("drop-crt");
    if let Err(e) = fs::create_dir_all(&shader_dir) {
        warn!("[RETROARCH] Failed to create bundled shader dir {}: {e}", shader_dir.display());
        return None;
    }

    let slang_preset = shader_dir.join("drop-crt.slangp");
    let glsl_preset = shader_dir.join("drop-crt.glslp");

    for (path, content, label) in [
        (shader_dir.join("drop-crt.slang"), DROP_CRT_SLANG, "slang shader"),
        (slang_preset.clone(), DROP_CRT_SLANGP, "slangp preset"),
        (shader_dir.join("drop-crt.glsl"), DROP_CRT_GLSL, "glsl shader"),
        (glsl_preset.clone(), DROP_CRT_GLSLP, "glslp preset"),
    ] {
        if let Err(e) = fs::write(&path, content) {
            warn!("[RETROARCH] Failed to write bundled {label}: {e}");
        }
    }

    info!("[RETROARCH] Wrote bundled CRT shader to {}", shader_dir.display());

    if slang_preset.is_file() {
        Some(slang_preset)
    } else if glsl_preset.is_file() {
        Some(glsl_preset)
    } else {
        None
    }
}

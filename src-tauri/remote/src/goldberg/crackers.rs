//! Multi-cracker achievement-file detection and parsing.
//!
//! Beyond Goldberg/GBE ([`super::achievements`]) and SmartSteamEmu
//! ([`super::sse`]), a downloaded game may ship with a *different* Steam
//! emulator / cracker that writes its unlock file to its own fixed location in
//! its own format. If Drop only looks at the Goldberg path, those unlocks are
//! invisible — which is the most common reason a game shows a silently-stuck
//! `0 / N` even though the player has unlocked achievements.
//!
//! This module ports the location map + per-format parsers from the Hydra
//! launcher (`find-achivement-files.ts` + `parse-achievement-file.ts`) so Drop
//! can read unlocks regardless of which cracker a game shipped with, plus the
//! **real Steam client** achievement cache for legitimately-owned games.
//!
//! All parsers normalise to [`GoldbergAchievement`] with `earned_time` in
//! **unix seconds** (matching Goldberg's format and the poll loop's
//! `[2000, 2100]` range validation in `remote/src/achievements.rs`).

use super::achievements::GoldbergAchievement;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::PathBuf;

/// On-disk achievement-file formats produced by the various crackers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrackerFormat {
    /// CODEX / RUNE: INI, per-achievement section with `Achieved=1` + `UnlockTime`.
    CodexIni,
    /// OnlineFix: INI, `achieved=true`/`Achieved=true` + `timestamp`/`TimeUnlocked`.
    OnlineFix,
    /// RLD! / dodi: INI, per-achievement hex-LE `State` (==1) + hex-LE `Time`.
    Rld,
    /// SKIDROW: INI `[Achievements]`, value `"1@...@<time>"`.
    Skidrow,
    /// 3DMGAME: INI `[State]` (=="0101") + `[Time]` (hex-LE seconds).
    ThreeDm,
    /// CreamAPI: INI/CFG, `achieved=true` + `unlocktime`.
    CreamApi,
    /// Razor1911: plain text lines `name unlocked unlockTime`.
    Razor1911,
    /// SteamEmu `user_stats.ini`: `[ACHIEVEMENTS]`, `"unlocked = true, time = <n>"`.
    UserStats,
    /// Goldberg / EMPRESS: JSON (map or definitions array) — reuses the GBE parser.
    GoldbergJson,
    /// The real Steam client cache: `librarycache/<appid>.json`.
    SteamCache,
}

/// A concrete achievement file found on disk for a game.
struct CrackerSource {
    label: &'static str,
    format: CrackerFormat,
    path: PathBuf,
}

// ── Base directories ─────────────────────────────────────────────────────

fn public_documents() -> PathBuf {
    // %PUBLIC% defaults to C:\Users\Public on Windows.
    match std::env::var("PUBLIC") {
        Ok(p) if !p.is_empty() => PathBuf::from(p).join("Documents"),
        _ => PathBuf::from("C:\\Users\\Public\\Documents"),
    }
}

fn program_data() -> PathBuf {
    match std::env::var("ProgramData") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from("C:\\ProgramData"),
    }
}

// ── Location map (ported from Hydra find-achivement-files.ts) ─────────────

/// Every cracker achievement file that *could* exist for `app_id`, across the
/// well-known fixed locations. Existence is checked by the caller; this just
/// enumerates the candidates + their format.
fn cracker_candidate_paths(app_id: &str) -> Vec<CrackerSource> {
    let app_data = dirs::data_dir();
    let local_app_data = dirs::data_local_dir();
    let documents = dirs::document_dir();
    let public_docs = public_documents();
    let program = program_data();

    let mut out: Vec<CrackerSource> = Vec::new();

    let mut push = |label: &'static str, format: CrackerFormat, path: PathBuf| {
        out.push(CrackerSource { label, format, path });
    };

    // CODEX — INI
    push("CODEX", CrackerFormat::CodexIni,
        public_docs.join("Steam").join("CODEX").join(app_id).join("achievements.ini"));
    if let Some(ad) = &app_data {
        push("CODEX", CrackerFormat::CodexIni,
            ad.join("Steam").join("CODEX").join(app_id).join("achievements.ini"));
    }

    // RUNE — INI (same shape as CODEX)
    push("RUNE", CrackerFormat::CodexIni,
        public_docs.join("Steam").join("RUNE").join(app_id).join("achievements.ini"));

    // OnlineFix — INI
    push("OnlineFix", CrackerFormat::OnlineFix,
        public_docs.join("OnlineFix").join(app_id).join("Stats").join("Achievements.ini"));
    push("OnlineFix", CrackerFormat::OnlineFix,
        public_docs.join("OnlineFix").join(app_id).join("Achievements.ini"));

    // RLD! / dodi — INI (hex-LE)
    push("RLD!", CrackerFormat::Rld,
        program.join("RLD!").join(app_id).join("achievements.ini"));
    push("RLD!", CrackerFormat::Rld,
        program.join("Steam").join("Player").join(app_id).join("stats").join("achievements.ini"));
    push("RLD!", CrackerFormat::Rld,
        program.join("Steam").join("RLD!").join(app_id).join("stats").join("achievements.ini"));
    push("dodi", CrackerFormat::Rld,
        program.join("Steam").join("dodi").join(app_id).join("stats").join("achievements.ini"));

    // EMPRESS — JSON (Goldberg-shaped)
    if let Some(ad) = &app_data {
        push("EMPRESS", CrackerFormat::GoldbergJson,
            ad.join("EMPRESS").join("remote").join(app_id).join("achievements.json"));
    }
    push("EMPRESS", CrackerFormat::GoldbergJson,
        public_docs.join("EMPRESS").join(app_id).join("remote").join(app_id).join("achievements.json"));

    // SKIDROW — INI
    if let Some(docs) = &documents {
        push("SKIDROW", CrackerFormat::Skidrow,
            docs.join("SKIDROW").join(app_id).join("SteamEmu").join("UserStats").join("achiev.ini"));
        push("SKIDROW", CrackerFormat::Skidrow,
            docs.join("Player").join(app_id).join("SteamEmu").join("UserStats").join("achiev.ini"));
    }
    if let Some(lad) = &local_app_data {
        push("SKIDROW", CrackerFormat::Skidrow,
            lad.join("SKIDROW").join(app_id).join("SteamEmu").join("UserStats").join("achiev.ini"));
    }

    // CreamAPI — CFG
    if let Some(ad) = &app_data {
        push("CreamAPI", CrackerFormat::CreamApi,
            ad.join("CreamAPI").join(app_id).join("stats").join("CreamAPI.Achievements.cfg"));
    }

    // Razor1911 — plain text
    if let Some(ad) = &app_data {
        push("Razor1911", CrackerFormat::Razor1911,
            ad.join(".1911").join(app_id).join("achievement"));
    }

    out
}

/// Cracker files that live *next to the game executable* (the DLL directory).
fn exe_adjacent_candidate_paths(dll_dir: &str) -> Vec<CrackerSource> {
    let base = PathBuf::from(dll_dir);
    vec![
        CrackerSource {
            label: "SteamData/user_stats",
            format: CrackerFormat::UserStats,
            path: base.join("SteamData").join("user_stats.ini"),
        },
        CrackerSource {
            label: "3DMGAME",
            format: CrackerFormat::ThreeDm,
            path: base.join("3DMGAME").join("Player").join("stats").join("achievements.ini"),
        },
    ]
}

// ── INI parsing ───────────────────────────────────────────────────────────

/// Parses INI text into `section -> (key -> value)`. Mirrors Hydra's `iniParse`
/// (BOM strip, `###` comments skipped, split on the first `=`).
fn ini_parse(contents: &str) -> HashMap<String, HashMap<String, String>> {
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut section = String::new();
    for raw in contents.split(['\r', '\n']) {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("###") {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            out.entry(section.clone()).or_default();
        } else if let Some((k, v)) = line.split_once('=') {
            out.entry(section.clone())
                .or_default()
                .insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    out
}

/// Decode a hex string (e.g. `"01000000"`) into a little-endian `u32`.
fn hex_le_u32(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.len() < 8 || !s.is_char_boundary(8) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..8)
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect();
    let b = bytes?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn earned(name: &str, earned_time: u64) -> GoldbergAchievement {
    GoldbergAchievement { name: name.to_string(), earned: true, earned_time }
}

// ── Per-format parsers (ported from Hydra parse-achievement-file.ts) ───────

/// CODEX / RUNE: each section is an achievement; `Achieved=1` + `UnlockTime`.
fn parse_codex(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    ini.iter()
        .filter(|(_, kv)| kv.get("Achieved").map(|s| s == "1").unwrap_or(false))
        .map(|(name, kv)| {
            let t = kv.get("UnlockTime").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            earned(name, t)
        })
        .collect()
}

/// OnlineFix: `achieved=true` (+`timestamp`) or `Achieved=true` (+`TimeUnlocked`).
fn parse_online_fix(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    let mut out = Vec::new();
    for (name, kv) in ini {
        if kv.get("achieved").map(|s| s == "true").unwrap_or(false) {
            let t = kv.get("timestamp").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            out.push(earned(name, t));
        } else if kv.get("Achieved").map(|s| s == "true").unwrap_or(false) {
            let t = kv.get("TimeUnlocked").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            out.push(earned(name, t));
        }
    }
    out
}

/// CreamAPI: `achieved=true` + `unlocktime`.
fn parse_cream_api(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    ini.iter()
        .filter(|(_, kv)| kv.get("achieved").map(|s| s == "true").unwrap_or(false))
        .map(|(name, kv)| {
            let t = kv.get("unlocktime").and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            earned(name, t)
        })
        .collect()
}

/// RLD! / dodi: per-achievement hex-LE `State` (==1 unlocked) + hex-LE `Time`.
fn parse_rld(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    let mut out = Vec::new();
    for (name, kv) in ini {
        if name == "Steam" {
            continue;
        }
        let unlocked = kv.get("State").and_then(|s| hex_le_u32(s)).map(|v| v == 1).unwrap_or(false);
        if unlocked {
            let t = kv.get("Time").and_then(|s| hex_le_u32(s)).unwrap_or(0) as u64;
            out.push(earned(name, t));
        }
    }
    out
}

/// SKIDROW: `[Achievements]` section, each value `"1@...@<time>"`.
fn parse_skidrow(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    let Some(ach) = ini.get("Achievements") else { return Vec::new() };
    let mut out = Vec::new();
    for (name, value) in ach {
        let parts: Vec<&str> = value.split('@').collect();
        if parts.first() == Some(&"1") {
            let t = parts.last().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
            out.push(earned(name, t));
        }
    }
    out
}

/// 3DMGAME: `[State]` (=="0101") + `[Time]` (hex-LE seconds).
fn parse_3dm(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    let Some(state) = ini.get("State") else { return Vec::new() };
    let times = ini.get("Time");
    let mut out = Vec::new();
    for (name, v) in state {
        if v == "0101" {
            let t = times
                .and_then(|m| m.get(name))
                .and_then(|s| hex_le_u32(s))
                .unwrap_or(0) as u64;
            out.push(earned(name, t));
        }
    }
    out
}

/// SteamEmu `user_stats.ini`: `[ACHIEVEMENTS]`, `"unlocked = true, time = <n>"`.
fn parse_user_stats(ini: &HashMap<String, HashMap<String, String>>) -> Vec<GoldbergAchievement> {
    let Some(ach) = ini.get("ACHIEVEMENTS") else { return Vec::new() };
    let mut out = Vec::new();
    for (name, value) in ach {
        // Value looks like `{unlocked = true, time = 1700000000}`.
        let inner = value.trim_matches(|c| c == '{' || c == '}');
        if let Some(idx) = inner.find("time = ")
            && let Ok(t) = inner[idx + "time = ".len()..].trim().parse::<u64>()
        {
            out.push(earned(name.trim_matches('"'), t));
        }
    }
    out
}

/// Razor1911: plain text lines `name unlocked unlockTime`.
fn parse_razor1911(contents: &str) -> Vec<GoldbergAchievement> {
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let mut out = Vec::new();
    for line in contents.split(['\r', '\n']) {
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split(' ').collect();
        if cols.len() >= 3 && cols[1] == "1" {
            let t = cols[2].parse::<u64>().unwrap_or(0);
            out.push(earned(cols[0], t));
        }
    }
    out
}

/// Goldberg / EMPRESS JSON — reuse the canonical GBE parser, then keep earned.
fn parse_goldberg_json(path: &std::path::Path) -> Vec<GoldbergAchievement> {
    // `read_goldberg_unlocks` takes an app_id + dll_dir and scans candidate
    // paths; here we already have the exact file, so parse it directly with the
    // shared typed parser by reading the file ourselves.
    let Ok(contents) = std::fs::read_to_string(path) else { return Vec::new() };
    // Map shape: {"ACH": {"earned": bool, "earned_time": n}}
    if let Ok(map) = serde_json::from_str::<HashMap<String, GbeEntry>>(&contents)
        && contents.trim_start().starts_with('{')
    {
        return map
            .into_iter()
            .filter(|(_, e)| e.earned)
            .map(|(name, e)| earned(&name, e.earned_time))
            .collect();
    }
    // Array shape: [{"name":..,"earned":..,"earned_time":..}]
    if let Ok(arr) = serde_json::from_str::<Vec<GoldbergAchievement>>(&contents) {
        return arr.into_iter().filter(|a| a.earned).collect();
    }
    Vec::new()
}

#[derive(serde::Deserialize)]
struct GbeEntry {
    #[serde(default)]
    earned: bool,
    #[serde(default)]
    earned_time: u64,
}

/// Real Steam client cache `librarycache/<appid>.json`: a top-level array where
/// one entry is `["achievements", {data:{vecHighlight:[{bAchieved,strID,rtUnlocked}]}}]`.
fn parse_steam_cache(path: &std::path::Path) -> Vec<GoldbergAchievement> {
    let Ok(contents) = std::fs::read_to_string(path) else { return Vec::new() };
    let Ok(root) = serde_json::from_str::<serde_json::Value>(&contents) else { return Vec::new() };
    let Some(entries) = root.as_array() else { return Vec::new() };

    let highlights = entries.iter().find_map(|e| {
        let pair = e.as_array()?;
        if pair.first()?.as_str()? == "achievements" {
            pair.get(1)?.get("data")?.get("vecHighlight")?.as_array()
        } else {
            None
        }
    });

    let Some(highlights) = highlights else { return Vec::new() };
    highlights
        .iter()
        .filter(|a| a.get("bAchieved").and_then(|v| v.as_bool()).unwrap_or(false))
        .filter_map(|a| {
            let name = a.get("strID")?.as_str()?.to_string();
            let t = a.get("rtUnlocked").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(GoldbergAchievement { name, earned: true, earned_time: t })
        })
        .collect()
}

// ── Dispatch + public API ──────────────────────────────────────────────────

fn parse_source(src: &CrackerSource) -> Vec<GoldbergAchievement> {
    let read_text = || std::fs::read_to_string(&src.path).ok();
    match src.format {
        CrackerFormat::CodexIni => read_text().map(|c| parse_codex(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::OnlineFix => read_text().map(|c| parse_online_fix(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::CreamApi => read_text().map(|c| parse_cream_api(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::Rld => read_text().map(|c| parse_rld(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::Skidrow => read_text().map(|c| parse_skidrow(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::ThreeDm => read_text().map(|c| parse_3dm(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::UserStats => read_text().map(|c| parse_user_stats(&ini_parse(&c))).unwrap_or_default(),
        CrackerFormat::Razor1911 => read_text().map(|c| parse_razor1911(&c)).unwrap_or_default(),
        CrackerFormat::GoldbergJson => parse_goldberg_json(&src.path),
        CrackerFormat::SteamCache => parse_steam_cache(&src.path),
    }
}

/// Locate an installed Steam client and return its per-user `librarycache`
/// achievement files for `app_id`. Best-effort path search (registry-free):
/// checks `STEAM_PATH`/`SteamPath` env and the common install dirs.
fn steam_cache_sources(app_id: &str) -> Vec<CrackerSource> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in ["STEAM_PATH", "SteamPath"] {
        if let Ok(p) = std::env::var(var)
            && !p.is_empty()
        {
            roots.push(PathBuf::from(p));
        }
    }
    for p in [
        "C:\\Program Files (x86)\\Steam",
        "C:\\Program Files\\Steam",
    ] {
        roots.push(PathBuf::from(p));
    }

    let mut out = Vec::new();
    for root in roots {
        let userdata = root.join("userdata");
        let Ok(users) = std::fs::read_dir(&userdata) else { continue };
        for user in users.flatten() {
            let p = user
                .path()
                .join("config")
                .join("librarycache")
                .join(format!("{app_id}.json"));
            if p.exists() {
                out.push(CrackerSource { label: "Steam", format: CrackerFormat::SteamCache, path: p });
            }
        }
    }
    out
}

/// Scan **every** known cracker / real-Steam location for `app_id` and return
/// the merged set of unlocked achievements (deduped by name, earliest unlock
/// time wins). This is the breadth that lets Drop surface achievements for
/// games cracked with something other than Goldberg/SSE.
pub fn scan_all_crackers(app_id: &str, dll_dir: Option<&str>) -> Vec<GoldbergAchievement> {
    const TAG: &str = "[ACH-CRACK]";

    let mut sources = cracker_candidate_paths(app_id);
    if let Some(dir) = dll_dir {
        sources.extend(exe_adjacent_candidate_paths(dir));
    }
    sources.extend(steam_cache_sources(app_id));

    let mut merged: HashMap<String, GoldbergAchievement> = HashMap::new();
    for src in &sources {
        if !src.path.exists() {
            continue;
        }
        let found = parse_source(src);
        if found.is_empty() {
            debug!("{TAG} {} at {} parsed 0 earned", src.label, src.path.display());
            continue;
        }
        info!("{TAG} {} ({:?}) at {}: {} earned", src.label, src.format, src.path.display(), found.len());
        for ach in found {
            merged
                .entry(ach.name.clone())
                .and_modify(|existing| {
                    // Keep the earliest non-zero unlock time.
                    if ach.earned_time != 0
                        && (existing.earned_time == 0 || ach.earned_time < existing.earned_time)
                    {
                        existing.earned_time = ach.earned_time;
                    }
                })
                .or_insert(ach);
        }
    }

    if merged.is_empty() {
        debug!("{TAG} No non-Goldberg cracker unlocks found for AppID {app_id}");
    } else {
        warn!("{TAG} Found {} unlocked achievements for AppID {app_id} via non-Goldberg crackers/Steam", merged.len());
    }
    merged.into_values().collect()
}

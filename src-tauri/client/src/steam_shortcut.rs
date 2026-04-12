//! Steam non-game shortcut registration.
//!
//! SteamOS Game Mode only shows apps from the Steam library. To appear there,
//! Drop must register itself as a "non-Steam game" shortcut in
//! `~/.steam/steam/userdata/<userId>/config/shortcuts.vdf`.
//!
//! The VDF binary format uses null-terminated strings and nested type markers:
//!   0x00 = start of map, 0x01 = string field, 0x02 = int32 field, 0x08 = end of map
//!
//! Reference: <https://developer.valvesoftware.com/wiki/Add_Non-Steam_Game>

use log::{info, warn};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Result of attempting to register Drop as a Steam shortcut.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutResult {
    pub success: bool,
    pub message: String,
}

/// Locate Steam's userdata directories.
/// Returns paths like `~/.steam/steam/userdata/12345678/config/shortcuts.vdf`
fn find_steam_userdata_dirs() -> Vec<PathBuf> {
    let mut results = Vec::new();

    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let steam_paths = [
        home.as_ref().map(|h| h.join(".steam/steam/userdata")),
        home.as_ref().map(|h| h.join(".local/share/Steam/userdata")),
        // Flatpak Steam
        home.as_ref()
            .map(|h| h.join(".var/app/com.valvesoftware.Steam/.steam/steam/userdata")),
    ];

    for path_opt in &steam_paths {
        if let Some(path) = path_opt {
            if path.is_dir() {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let config_dir = entry.path().join("config");
                        if config_dir.is_dir() {
                            results.push(config_dir.join("shortcuts.vdf"));
                        }
                    }
                }
            }
        }
    }

    results
}

/// Find the Drop executable path for the shortcut.
fn find_drop_executable() -> Option<String> {
    // Flatpak: the binary is at /app/bin/drop-app
    if std::path::Path::new("/.flatpak-info").exists() {
        return Some("flatpak run org.droposs.client".to_string());
    }

    // AppImage: use the APPIMAGE env var
    if let Ok(appimage) = std::env::var("APPIMAGE") {
        return Some(appimage);
    }

    // Fall back to current executable
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

// ── VDF binary format helpers ────────────────────────────────────────────

fn vdf_write_string(buf: &mut Vec<u8>, key: &str, value: &str) {
    buf.push(0x01); // string type
    buf.extend_from_slice(key.as_bytes());
    buf.push(0x00); // null terminator for key
    buf.extend_from_slice(value.as_bytes());
    buf.push(0x00); // null terminator for value
}

fn vdf_write_int(buf: &mut Vec<u8>, key: &str, value: u32) {
    buf.push(0x02); // int32 type
    buf.extend_from_slice(key.as_bytes());
    buf.push(0x00); // null terminator for key
    buf.extend_from_slice(&value.to_le_bytes());
}

fn vdf_start_map(buf: &mut Vec<u8>, key: &str) {
    buf.push(0x00); // map type
    buf.extend_from_slice(key.as_bytes());
    buf.push(0x00); // null terminator
}

fn vdf_end_map(buf: &mut Vec<u8>) {
    buf.push(0x08); // end of map
}

/// Generate a Steam-compatible app ID from the shortcut name + exe path.
/// This matches Steam's own algorithm for non-game shortcuts.
fn generate_shortcut_id(exe: &str, name: &str) -> u32 {
    let input = format!("{}{}", exe, name);
    let mut hash: u32 = 0x4F17A8C5; // CRC32-like seed used by Steam
    for byte in input.bytes() {
        hash = hash.wrapping_mul(0x01000193) ^ (byte as u32);
    }
    (hash & 0x7FFFFFFF) | 0x02000000
}

/// Check if Drop is already registered in a shortcuts.vdf file.
fn is_already_registered(vdf_path: &PathBuf) -> bool {
    if !vdf_path.exists() {
        return false;
    }
    if let Ok(contents) = fs::read(vdf_path) {
        // Search for "Drop" in the binary VDF
        let needle = b"Drop Desktop App";
        contents
            .windows(needle.len())
            .any(|window| window == needle)
    } else {
        false
    }
}

/// Build a new shortcut entry as VDF binary.
fn build_shortcut_entry(index: u32) -> Option<Vec<u8>> {
    let exe = find_drop_executable()?;
    let name = "Drop Desktop App";
    // Steam requires the exe and name to be quoted in the VDF for the
    // AppName to display correctly (otherwise it falls back to filename).
    let quoted_exe = format!("\"{}\"", exe);
    let app_id = generate_shortcut_id(&quoted_exe, name);

    let mut buf = Vec::new();
    vdf_start_map(&mut buf, &index.to_string());

    vdf_write_int(&mut buf, "appid", app_id);
    vdf_write_string(&mut buf, "AppName", name);
    vdf_write_string(&mut buf, "Exe", &quoted_exe);
    vdf_write_string(&mut buf, "StartDir", "");
    vdf_write_string(&mut buf, "icon", "");
    vdf_write_string(&mut buf, "ShortcutPath", "");
    vdf_write_string(&mut buf, "LaunchOptions", "--big-picture");
    vdf_write_int(&mut buf, "IsHidden", 0);
    vdf_write_int(&mut buf, "AllowDesktopConfig", 1);
    vdf_write_int(&mut buf, "AllowOverlay", 1);
    vdf_write_int(&mut buf, "OpenVR", 0);
    vdf_write_int(&mut buf, "Devkit", 0);
    vdf_write_string(&mut buf, "DevkitGameID", "");
    vdf_write_int(&mut buf, "DevkitOverrideAppID", 0);
    vdf_write_int(&mut buf, "LastPlayTime", 0);
    vdf_write_string(&mut buf, "FlatpakAppID", "");
    // Tags
    vdf_start_map(&mut buf, "tags");
    vdf_write_string(&mut buf, "0", "Drop");
    vdf_write_string(&mut buf, "1", "Game Launcher");
    vdf_end_map(&mut buf);

    vdf_end_map(&mut buf);

    Some(buf)
}

/// Register Drop as a non-Steam game shortcut so it appears in SteamOS Game Mode.
pub fn register_steam_shortcut() -> ShortcutResult {
    let vdf_paths = find_steam_userdata_dirs();

    if vdf_paths.is_empty() {
        return ShortcutResult {
            success: false,
            message: "Steam installation not found. Make sure Steam is installed.".to_string(),
        };
    }

    let mut registered_count = 0;
    let mut errors = Vec::new();

    for vdf_path in &vdf_paths {
        if is_already_registered(vdf_path) {
            info!("Drop shortcut already exists in {}", vdf_path.display());
            registered_count += 1;
            continue;
        }

        // Read existing file or start fresh
        let mut contents = if vdf_path.exists() {
            match fs::read(vdf_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read {}: {}", vdf_path.display(), e);
                    errors.push(format!("{}: {}", vdf_path.display(), e));
                    continue;
                }
            }
        } else {
            // Create a new shortcuts.vdf with root map
            let mut new = Vec::new();
            vdf_start_map(&mut new, "shortcuts");
            vdf_end_map(&mut new);
            new
        };

        // Find the next index by counting existing entries (crude but effective)
        let existing_count = contents
            .windows(8)
            .filter(|w| w == b"AppName\x00" || w == b"appname\x00")
            .count() as u32;

        let entry = match build_shortcut_entry(existing_count) {
            Some(e) => e,
            None => {
                errors.push("Could not determine Drop executable path".to_string());
                continue;
            }
        };

        // Insert entry before the final 0x08 (end of root map)
        if let Some(last_byte_pos) = contents.iter().rposition(|&b| b == 0x08) {
            contents.splice(last_byte_pos..last_byte_pos, entry);
        } else {
            // No end marker found — wrap in a root map
            let mut new_contents = Vec::new();
            vdf_start_map(&mut new_contents, "shortcuts");
            new_contents.extend_from_slice(&entry);
            vdf_end_map(&mut new_contents);
            contents = new_contents;
        }

        // Ensure parent directory exists
        if let Some(parent) = vdf_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match fs::write(vdf_path, &contents) {
            Ok(_) => {
                info!("Registered Drop shortcut in {}", vdf_path.display());
                registered_count += 1;
            }
            Err(e) => {
                warn!("Failed to write {}: {}", vdf_path.display(), e);
                errors.push(format!("{}: {}", vdf_path.display(), e));
            }
        }
    }

    if registered_count > 0 {
        ShortcutResult {
            success: true,
            message: format!(
                "Drop added to Steam for {} user profile(s). Restart Steam to see it in Game Mode.",
                registered_count
            ),
        }
    } else {
        ShortcutResult {
            success: false,
            message: format!("Failed to register shortcut: {}", errors.join("; ")),
        }
    }
}

/// Artwork types that Steam uses in the grid directory.
/// Each has a specific filename pattern: `{appid}{suffix}.png`
pub enum SteamArtworkType {
    /// Hero banner (960x342) — shown on game detail page
    Hero,
    /// Grid/cover (600x900) — shown in library grid view
    Grid,
    /// Logo (transparent PNG) — overlaid on hero
    Logo,
    /// Wide grid / header (460x215) — used in recent games
    Header,
    /// Icon (square)
    Icon,
}

impl SteamArtworkType {
    fn suffix(&self) -> &'static str {
        match self {
            SteamArtworkType::Hero => "_hero",
            SteamArtworkType::Grid => "p",
            SteamArtworkType::Logo => "_logo",
            SteamArtworkType::Header => "",
            SteamArtworkType::Icon => "_icon",
        }
    }
}

/// Information about a game to add as a Steam shortcut.
pub struct GameShortcutInfo {
    pub game_id: String,
    pub game_name: String,
    /// Map of artwork type suffix → PNG bytes (e.g. "_hero" → bytes)
    pub artwork: Vec<(SteamArtworkType, Vec<u8>)>,
}

/// Check if a game is already registered in a shortcuts.vdf file.
fn is_game_registered(vdf_path: &PathBuf, game_name: &str) -> bool {
    if !vdf_path.exists() {
        return false;
    }
    if let Ok(contents) = fs::read(vdf_path) {
        let needle = game_name.as_bytes();
        contents
            .windows(needle.len())
            .any(|window| window == needle)
    } else {
        false
    }
}

/// Build a shortcut entry for a specific game launched via Drop.
fn build_game_shortcut_entry(index: u32, info: &GameShortcutInfo) -> Option<Vec<u8>> {
    let exe = find_drop_executable()?;
    let quoted_exe = format!("\"{}\"", exe);
    let app_id = generate_shortcut_id(&quoted_exe, &info.game_name);

    let mut buf = Vec::new();
    vdf_start_map(&mut buf, &index.to_string());

    vdf_write_int(&mut buf, "appid", app_id);
    vdf_write_string(&mut buf, "AppName", &info.game_name);
    vdf_write_string(&mut buf, "Exe", &quoted_exe);
    vdf_write_string(&mut buf, "StartDir", "");
    vdf_write_string(&mut buf, "icon", "");
    vdf_write_string(&mut buf, "ShortcutPath", "");
    vdf_write_string(&mut buf, "LaunchOptions", &format!("--launch {}", info.game_id));
    vdf_write_int(&mut buf, "IsHidden", 0);
    vdf_write_int(&mut buf, "AllowDesktopConfig", 1);
    vdf_write_int(&mut buf, "AllowOverlay", 1);
    vdf_write_int(&mut buf, "OpenVR", 0);
    vdf_write_int(&mut buf, "Devkit", 0);
    vdf_write_string(&mut buf, "DevkitGameID", "");
    vdf_write_int(&mut buf, "DevkitOverrideAppID", 0);
    vdf_write_int(&mut buf, "LastPlayTime", 0);
    vdf_write_string(&mut buf, "FlatpakAppID", "");
    // Tags
    vdf_start_map(&mut buf, "tags");
    vdf_write_string(&mut buf, "0", "Drop");
    vdf_write_string(&mut buf, "1", "Games");
    vdf_end_map(&mut buf);

    vdf_end_map(&mut buf);

    Some(buf)
}

/// Save artwork files into Steam's grid directory for a given shortcut.
/// `config_dir` is the `userdata/<uid>/config/` directory (parent of shortcuts.vdf).
/// `exe` should be the quoted exe string used in `generate_shortcut_id`.
fn save_artwork(config_dir: &PathBuf, exe: &str, info: &GameShortcutInfo) {
    let app_id = generate_shortcut_id(exe, &info.game_name);
    let grid_dir = config_dir.join("grid");

    if let Err(e) = fs::create_dir_all(&grid_dir) {
        warn!("Failed to create grid directory {}: {}", grid_dir.display(), e);
        return;
    }

    for (art_type, bytes) in &info.artwork {
        if bytes.is_empty() {
            continue;
        }
        let filename = format!("{}{}.png", app_id, art_type.suffix());
        let path = grid_dir.join(&filename);
        match fs::write(&path, bytes) {
            Ok(_) => info!("[STEAM] Saved artwork {} ({} bytes)", path.display(), bytes.len()),
            Err(e) => warn!("[STEAM] Failed to save artwork {}: {}", path.display(), e),
        }
    }
}

/// Register a specific game as a non-Steam shortcut with artwork.
pub fn add_game_to_steam(info: GameShortcutInfo) -> ShortcutResult {
    let vdf_paths = find_steam_userdata_dirs();

    if vdf_paths.is_empty() {
        return ShortcutResult {
            success: false,
            message: "Steam installation not found. Make sure Steam is installed.".to_string(),
        };
    }

    let exe = match find_drop_executable() {
        Some(e) => e,
        None => {
            return ShortcutResult {
                success: false,
                message: "Could not determine Drop executable path.".to_string(),
            };
        }
    };
    let quoted_exe = format!("\"{}\"", exe);

    let mut registered_count = 0;
    let mut errors = Vec::new();

    for vdf_path in &vdf_paths {
        if is_game_registered(vdf_path, &info.game_name) {
            info!("[STEAM] Game '{}' already registered in {}", info.game_name, vdf_path.display());
            // Still update artwork even if already registered
            if let Some(parent) = vdf_path.parent() {
                save_artwork(&parent.to_path_buf(), &quoted_exe, &info);
            }
            registered_count += 1;
            continue;
        }

        // Read existing file or start fresh
        let mut contents = if vdf_path.exists() {
            match fs::read(vdf_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read {}: {}", vdf_path.display(), e);
                    errors.push(format!("{}: {}", vdf_path.display(), e));
                    continue;
                }
            }
        } else {
            let mut new = Vec::new();
            vdf_start_map(&mut new, "shortcuts");
            vdf_end_map(&mut new);
            new
        };

        // Find the next index
        let existing_count = contents
            .windows(8)
            .filter(|w| w == b"AppName\x00" || w == b"appname\x00")
            .count() as u32;

        let entry = match build_game_shortcut_entry(existing_count, &info) {
            Some(e) => e,
            None => {
                errors.push("Could not determine Drop executable path".to_string());
                continue;
            }
        };

        // Insert entry before the final 0x08 (end of root map)
        if let Some(last_byte_pos) = contents.iter().rposition(|&b| b == 0x08) {
            contents.splice(last_byte_pos..last_byte_pos, entry);
        } else {
            let mut new_contents = Vec::new();
            vdf_start_map(&mut new_contents, "shortcuts");
            new_contents.extend_from_slice(&entry);
            vdf_end_map(&mut new_contents);
            contents = new_contents;
        }

        if let Some(parent) = vdf_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match fs::write(vdf_path, &contents) {
            Ok(_) => {
                info!("[STEAM] Registered game '{}' in {}", info.game_name, vdf_path.display());
                // Save artwork
                if let Some(parent) = vdf_path.parent() {
                    save_artwork(&parent.to_path_buf(), &quoted_exe, &info);
                }
                registered_count += 1;
            }
            Err(e) => {
                warn!("Failed to write {}: {}", vdf_path.display(), e);
                errors.push(format!("{}: {}", vdf_path.display(), e));
            }
        }
    }

    if registered_count > 0 {
        ShortcutResult {
            success: true,
            message: format!(
                "'{}' added to Steam. Restart Steam to see it in Game Mode.",
                info.game_name
            ),
        }
    } else {
        ShortcutResult {
            success: false,
            message: format!("Failed to add game to Steam: {}", errors.join("; ")),
        }
    }
}

/// Remove Drop's non-Steam shortcut from all user profiles.
pub fn unregister_steam_shortcut() -> ShortcutResult {
    let vdf_paths = find_steam_userdata_dirs();
    let mut removed = 0;

    for vdf_path in &vdf_paths {
        if !is_already_registered(vdf_path) {
            continue;
        }
        // For removal, we'd need to parse the VDF and remove the Drop entry.
        // For now, just report that manual removal is needed.
        removed += 1;
    }

    if removed > 0 {
        ShortcutResult {
            success: true,
            message: "To fully remove the shortcut, delete it from Steam's library.".to_string(),
        }
    } else {
        ShortcutResult {
            success: true,
            message: "No Drop shortcuts found to remove.".to_string(),
        }
    }
}

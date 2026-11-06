//! Read-only native Steam cache adapter. Binary KeyValues formats are undocumented;
//! reject unknown/truncated encodings instead of returning an empty achievement list.
use super::tracking::{Sample, Snapshot};
use serde_json::{Map, Value};
use std::{fs, path::PathBuf};

const MAX_BYTES: usize = 32 * 1024 * 1024;

fn binary_kv(data: &[u8]) -> Result<Value, String> {
    fn string(data: &[u8], pos: &mut usize) -> Result<String, String> {
        let tail = data.get(*pos..).ok_or("Truncated Steam cache")?;
        let len = tail
            .iter()
            .position(|b| *b == 0)
            .ok_or("Unterminated Steam cache string")?;
        let text = std::str::from_utf8(&tail[..len])
            .map_err(|_| "Invalid Steam cache text")?
            .to_string();
        *pos += len + 1;
        Ok(text)
    }
    fn bytes<const N: usize>(data: &[u8], pos: &mut usize) -> Result<[u8; N], String> {
        let slice = data
            .get(*pos..*pos + N)
            .ok_or("Truncated Steam cache value")?;
        let mut out = [0; N];
        out.copy_from_slice(slice);
        *pos += N;
        Ok(out)
    }
    fn object(
        data: &[u8],
        pos: &mut usize,
        depth: usize,
        count: &mut usize,
    ) -> Result<Value, String> {
        if depth > 32 {
            return Err("Steam cache nesting limit".into());
        }
        let mut map = Map::new();
        loop {
            let kind = *data.get(*pos).ok_or("Truncated Steam cache object")?;
            *pos += 1;
            if kind == 8 {
                return Ok(Value::Object(map));
            }
            *count += 1;
            if *count > 100_000 {
                return Err("Steam cache entry limit".into());
            }
            let key = string(data, pos)?;
            let value = match kind {
                0 => object(data, pos, depth + 1, count)?,
                1 => Value::String(string(data, pos)?),
                2 => Value::from(u32::from_le_bytes(bytes(data, pos)?)),
                3 => {
                    let f = f32::from_le_bytes(bytes(data, pos)?) as f64;
                    if !f.is_finite() {
                        return Err("Non-finite Steam stat".into());
                    }
                    Value::from(f)
                }
                7 => Value::from(u64::from_le_bytes(bytes(data, pos)?)),
                _ => return Err(format!("Unsupported Steam cache type {kind}")),
            };
            map.insert(key, value);
        }
    }
    if data.len() > MAX_BYTES {
        return Err("Steam cache exceeds size limit".into());
    }
    let mut pos = 0;
    let value = object(data, &mut pos, 0, &mut 0)?;
    if data[pos..].iter().any(|b| *b != 8) {
        return Err("Unexpected trailing Steam cache data".into());
    }
    Ok(value)
}

fn number(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

fn decode(schema: &[u8], stats: &[u8], app_id: &str, account: &str) -> Result<Vec<Sample>, String> {
    let schema = binary_kv(schema)?;
    let data = binary_kv(stats)?;
    let groups = schema
        .get(app_id)
        .and_then(|v| v.get("stats"))
        .and_then(|v| v.as_object())
        .ok_or("Steam schema does not match this game")?;
    let cache = data
        .get("cache")
        .and_then(|v| v.as_object())
        .ok_or("Steam cache has no stats object")?;
    let mut out = Vec::new();
    for (group_id, group) in groups {
        let Some(bits) = group.get("bits").and_then(|v| v.as_object()) else {
            continue;
        };
        let state = cache.get(group_id);
        let mask = state
            .and_then(|v| v.get("data"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        for (bit, definition) in bits {
            let index = bit
                .parse::<u32>()
                .map_err(|_| "Invalid Steam achievement bit")?;
            if index >= 32 {
                return Err("Unsupported Steam achievement bit width".into());
            }
            let name = definition
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Steam achievement has no API name")?;
            let earned = mask & (1u64 << index) != 0;
            let earned_time = state
                .and_then(|v| v.get("AchievementTimes"))
                .and_then(|v| v.get(bit))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let progress = definition.get("progress");
            let stat_name = progress
                .and_then(|v| v.get("value"))
                .and_then(|v| v.get("operand1"))
                .and_then(|v| v.as_str());
            let stat_id = stat_name.and_then(|name| {
                groups
                    .iter()
                    .find(|(_, v)| v.get("name").and_then(|v| v.as_str()) == Some(name))
                    .map(|(id, _)| id)
            });
            let current = stat_id
                .and_then(|id| cache.get(id))
                .and_then(|v| v.get("data"))
                .and_then(number);
            let target = progress.and_then(|v| v.get("max_val")).and_then(number);
            out.push(Sample {
                app_id: app_id.into(),
                account: account.into(),
                source: "steam-native".into(),
                name: name.into(),
                earned,
                earned_time,
                current,
                target,
            });
        }
    }
    Ok(out)
}

/// Parse only quoted tokens and braces, without ever logging the account file contents.
fn recent_account(text: &str) -> Result<String, String> {
    let mut tokens = Vec::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            let mut token = String::new();
            let mut closed = false;
            while let Some(c) = chars.next() {
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    token.push(chars.next().ok_or("Incomplete Steam account file")?);
                } else {
                    token.push(c);
                }
            }
            if !closed {
                return Err("Incomplete Steam account file".into());
            }
            tokens.push(token);
        } else if c == '{' || c == '}' {
            tokens.push(c.to_string());
        }
    }
    let mut active = Vec::new();
    let mut account = None;
    let mut depth = 0;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "{" => {
                depth += 1;
            }
            "}" => {
                if depth == 2 {
                    account = None;
                }
                depth -= 1;
            }
            key if depth == 1 && key.parse::<u64>().is_ok() => {
                account = Some(key.to_string());
            }
            "MostRecent" if depth == 2 && tokens.get(i + 1).is_some_and(|s| s == "1") => {
                if let Some(id) = &account {
                    active.push(id.clone());
                }
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    if depth != 0 || active.len() != 1 {
        return Err("Steam active account is ambiguous".into());
    }
    Ok(active.remove(0))
}

fn roots() -> &'static [PathBuf] {
    static ROOTS: std::sync::OnceLock<Vec<PathBuf>> = std::sync::OnceLock::new();
    ROOTS.get_or_init(discover_roots)
}

fn discover_roots() -> Vec<PathBuf> {
    let mut paths = super::WindowsRoots::host().steam_dirs;
    if let Some(home) = dirs::home_dir() {
        paths.extend([home.join(".steam/steam"), home.join(".local/share/Steam")]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if let Ok(output) = std::process::Command::new("reg.exe")
            .args(["query", "HKCU\\Software\\Valve\\Steam", "/v", "SteamPath"])
            .creation_flags(0x08000000)
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = text
                .lines()
                .find_map(|l| l.split_once("REG_SZ").map(|(_, p)| p.trim()))
            {
                paths.insert(0, path.into());
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(fs::canonicalize(p).unwrap_or_else(|_| p.clone())));
    // Bind one Steam installation, preferring its registered path. Never merge libraries/accounts
    // from stale additional Steam installations.
    if let Some(selected) = paths
        .iter()
        .find(|p| p.join("config/loginusers.vdf").is_file())
        .cloned()
    {
        return vec![selected];
    }
    paths
}

pub fn snapshot(app_id: &str) -> Snapshot {
    let mut out = Snapshot {
        samples: vec![],
        paths: vec![],
        errors: vec![],
    };
    if app_id.parse::<u32>().is_err() {
        return out;
    }
    for root in roots() {
        let schema = root
            .join("appcache/stats")
            .join(format!("UserGameStatsSchema_{app_id}.bin"));
        if !schema.exists() {
            continue;
        }
        let login = root.join("config/loginusers.vdf");
        out.paths.push(login.clone());
        let account = match fs::read_to_string(login)
            .map_err(|e| e.to_string())
            .and_then(|text| recent_account(&text))
        {
            Ok(id) => id,
            Err(e) => {
                out.errors.push(e);
                continue;
            }
        };
        let Some(account_id) = account
            .parse::<u64>()
            .ok()
            .and_then(|id| id.checked_sub(76561197960265728))
        else {
            out.errors.push("Invalid Steam account identifier".into());
            continue;
        };
        out.paths.push(schema.clone());
        let candidates = [account_id.to_string(), account.clone()];
        for user in candidates {
            let data = root
                .join("appcache/stats")
                .join(format!("UserGameStats_{user}_{app_id}.bin"));
            out.paths.push(data.clone());
            if !data.exists() {
                continue;
            }
            let parsed = fs::read(&schema)
                .and_then(|s| fs::read(&data).map(|d| (s, d)))
                .map_err(|e| e.to_string())
                .and_then(|(s, d)| decode(&s, &d, app_id, &account));
            match parsed {
                Ok(samples) => out.samples.extend(samples),
                Err(e) => out.errors.push(e),
            }
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn encode(value: &Value) -> Vec<u8> {
        let mut out = vec![];
        for (k, v) in value.as_object().unwrap() {
            out.push(if v.is_object() {
                0
            } else if v.is_string() {
                1
            } else {
                2
            });
            out.extend(k.bytes());
            out.push(0);
            if v.is_object() {
                out.extend(encode(v));
            } else if let Some(s) = v.as_str() {
                out.extend(s.bytes());
                out.push(0);
            } else {
                out.extend((v.as_u64().unwrap() as u32).to_le_bytes());
            }
        }
        out.push(8);
        out
    }
    #[test]
    fn complete_native_snapshot_decodes_bitfields_and_keeps_account_identity() {
        let schema = serde_json::json!({"10":{"stats":{"1":{"bits":{"0":{"name":"first"},"1":{"name":"second"}}}}}});
        let stats =
            serde_json::json!({"cache":{"1":{"data":2,"AchievementTimes":{"1":1700000000}}}});
        let samples = decode(&encode(&schema), &encode(&stats), "10", "alice").unwrap();
        assert_eq!(samples.len(), 2);
        assert!(!samples[0].earned);
        assert!(samples[1].earned);
        assert_eq!(samples[1].earned_time, 1700000000);
        assert_eq!(samples[1].account, "alice");
        assert!(binary_kv(&[0, b'x', 0, 2]).is_err());
        assert!(binary_kv(&[9, 0]).is_err());
    }
    #[test]
    fn multiple_accounts_are_never_merged_or_chosen_by_directory_order() {
        let users = r#""users" { "76561198000000001" { "MostRecent" "0" } "76561198000000002" { "MostRecent" "1" } }"#;
        assert_eq!(recent_account(users).unwrap(), "76561198000000002");
        assert!(recent_account(&users.replace("\"0\"", "\"1\"")).is_err());
    }
}

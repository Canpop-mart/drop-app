use std::{
    fs,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use keyring::Entry;
use log::{info, warn};

use crate::interface::DatabaseInterface;

pub static DB: LazyLock<DatabaseInterface> = LazyLock::new(DatabaseInterface::set_up_database);

#[cfg(not(debug_assertions))]
static DATA_ROOT_PREFIX: &str = "drop";
#[cfg(debug_assertions)]
static DATA_ROOT_PREFIX: &str = "drop-debug";

pub static DATA_ROOT_DIR: LazyLock<Arc<PathBuf>> = LazyLock::new(|| {
    Arc::new(
        dirs::data_dir()
            .expect("Failed to get data dir")
            .join(DATA_ROOT_PREFIX),
    )
});

/// Load or generate the database encryption key+IV.
///
/// The file-based key (`db.key`) is the **canonical** source of truth.
/// If it exists, we always use it — this avoids corruption caused by
/// the system keyring being intermittently available (common in AppImage
/// and SteamOS environments where D-Bus / Secret Service may not be
/// ready on every launch).
///
/// Key lookup order:
/// 1. File-based key (`db.key`) — if it exists and is valid, use it.
/// 2. System keyring — if file key doesn't exist, try keyring.
/// 3. Generate new key — save to file (and optionally keyring).
fn load_or_generate_key() -> ([u8; 16], [u8; 16]) {
    let key_path = DATA_ROOT_DIR.join("db.key");

    // 1. Always prefer the file-based key if it exists — it's deterministic
    //    and immune to keyring availability issues.
    if let Ok(data) = fs::read(&key_path) {
        if data.len() >= 32 {
            info!("loaded database key from file (canonical source)");
            return split_key_iv(&data);
        }
        warn!("key file exists but is too short ({} bytes), will regenerate", data.len());
    }

    // 2. No file key — try migrating from keyring (for users upgrading from
    //    older versions that only stored the key in the keyring).
    if let Ok(entry) = Entry::new("drop", "database_key") {
        match entry.get_secret() {
            Ok(secret) if secret.len() >= 32 => {
                info!("migrating database key from system keyring to file");
                // Write the keyring key to file so we never depend on keyring again
                write_key_file(&key_path, &secret);
                return split_key_iv(&secret);
            }
            Ok(_) => {
                warn!("keyring secret too short, generating fresh key");
            }
            Err(_) => {
                info!("no key in keyring either, generating fresh key");
            }
        }
    } else {
        info!("keyring unavailable, generating fresh key");
    }

    // 3. Generate a brand-new key and save to file (primary) + keyring (best-effort)
    let buffer = generate_random_key();
    write_key_file(&key_path, &buffer);

    // Best-effort: also store in keyring for discoverability, but file is canonical
    if let Ok(entry) = Entry::new("drop", "database_key") {
        if let Err(e) = entry.set_secret(&buffer) {
            info!("could not cache key in keyring (non-fatal): {e}");
        }
    }

    info!("created new database key in file: {}", key_path.display());
    split_key_iv(&buffer)
}

fn generate_random_key() -> Vec<u8> {
    let mut buffer = [0u8; 32];
    rand::fill(&mut buffer);
    buffer.to_vec()
}

fn split_key_iv(data: &[u8]) -> ([u8; 16], [u8; 16]) {
    (
        data[0..16].try_into().expect("key wrong length"),
        data[16..32].try_into().expect("iv wrong length"),
    )
}

/// Write a key to the file-based key store with restrictive permissions.
fn write_key_file(key_path: &std::path::Path, key_data: &[u8]) {
    // Ensure parent directory exists
    if let Some(parent) = key_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    fs::write(key_path, key_data).expect("failed to write database key file");

    // Set restrictive permissions on Unix (0600 = owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(key_path, perms);
    }
}

pub(crate) static KEY_IV: LazyLock<([u8; 16], [u8; 16])> = LazyLock::new(load_or_generate_key);
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
/// Tries the system keyring first; falls back to a file-based key
/// stored alongside the database with restrictive permissions.
fn load_or_generate_key() -> ([u8; 16], [u8; 16]) {
    // Try the system keyring first
    if let Ok(entry) = Entry::new("drop", "database_key") {
        match entry.get_secret() {
            Ok(secret) if secret.len() >= 32 => {
                info!("loaded database key from system keyring");
                return split_key_iv(&secret);
            }
            Ok(_) => {
                warn!("keyring secret too short, regenerating");
            }
            Err(_) => {
                // Key not yet stored — generate and try to save
                let buffer = generate_random_key();
                match entry.set_secret(&buffer) {
                    Ok(()) => {
                        info!("created new database key in system keyring");
                        return split_key_iv(&buffer);
                    }
                    Err(e) => {
                        warn!("failed to save key to keyring: {e}, falling back to file-based key");
                    }
                }
            }
        }
    } else {
        warn!("failed to open keyring, falling back to file-based key");
    }

    // Fallback: file-based key stored next to the database
    load_or_generate_file_key()
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

fn load_or_generate_file_key() -> ([u8; 16], [u8; 16]) {
    let key_path = DATA_ROOT_DIR.join("db.key");

    // Try to read existing key file
    if let Ok(data) = fs::read(&key_path) {
        if data.len() >= 32 {
            info!("loaded database key from file");
            return split_key_iv(&data);
        }
        warn!("key file too short, regenerating");
    }

    // Generate new key and write to file
    let buffer = generate_random_key();

    // Ensure parent directory exists
    if let Some(parent) = key_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    fs::write(&key_path, &buffer).expect("failed to write database key file");

    // Set restrictive permissions on Unix (0600 = owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(&key_path, perms);
    }

    info!("created new database key in file: {}", key_path.display());
    split_key_iv(&buffer)
}

pub(crate) static KEY_IV: LazyLock<([u8; 16], [u8; 16])> = LazyLock::new(load_or_generate_key);
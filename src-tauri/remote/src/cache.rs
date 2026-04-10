use std::{
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
    collections::HashMap,
};

use bitcode::{Decode, DecodeOwned, Encode};
use database::{Database, borrow_db_checked};
use http::{Response, header::CONTENT_TYPE, response::Builder as ResponseBuilder};
use once_cell::sync::Lazy;

use crate::error::{CacheError, RemoteAccessError};

/// In-memory cache entry with expiry time
#[derive(Clone)]
struct MemoryCacheEntry {
    data: Vec<u8>,
    expiry: u64,
}

/// In-memory cache with max 100 entries (LRU policy will be handled by simple limit)
static MEMORY_CACHE: Lazy<Arc<Mutex<HashMap<String, MemoryCacheEntry>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

const MAX_MEMORY_CACHE_SIZE: usize = 100;

#[macro_export]
macro_rules! offline {
    ($var:expr, $func1:expr, $func2:expr, $( $arg:expr ),* ) => {

        async move {
            if ::database::borrow_db_checked().settings.force_offline
            || $var.lock().status == ::client::app_status::AppStatus::Offline {
            $func2( $( $arg ), *).await
        } else {
            $func1( $( $arg ), *).await
        }
        }
    }
}

fn get_sys_time_in_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn get_cache_path(base: &Path, key: &str) -> PathBuf {
    let key_hash = hex::encode(md5::compute(key.as_bytes()).0);
    base.join(key_hash)
}

fn write_sync(base: &Path, key: &str, data: Vec<u8>) -> io::Result<()> {
    let cache_path = get_cache_path(base, key);
    let mut file = File::create(cache_path)?;
    file.write_all(&data)?;
    Ok(())
}

fn read_sync(base: &Path, key: &str) -> io::Result<Vec<u8>> {
    let cache_path = get_cache_path(base, key);
    let file = std::fs::read(cache_path)?;
    Ok(file)
}

fn delete_sync(base: &Path, key: &str) -> io::Result<()> {
    let cache_path = get_cache_path(base, key);
    std::fs::remove_file(cache_path)?;
    Ok(())
}

pub fn cache_object<D: Encode>(key: &str, data: &D) -> Result<(), RemoteAccessError> {
    cache_object_db(key, data, &borrow_db_checked())
}

/// Write to both memory and disk (write-through policy)
pub fn cache_object_db<D: Encode>(
    key: &str,
    data: &D,
    database: &Database,
) -> Result<(), RemoteAccessError> {
    let bytes = bitcode::encode(data);

    // Write to disk
    write_sync(&database.cache_dir, key, bytes.clone()).map_err(RemoteAccessError::Cache)?;

    // Write to memory cache with default 24 hour expiry
    let expiry = get_sys_time_in_secs() + 60 * 60 * 24;
    store_in_memory_cache(key.to_string(), bytes, expiry);

    Ok(())
}
pub fn get_cached_object<D: Encode + DecodeOwned>(key: &str) -> Result<D, RemoteAccessError> {
    get_cached_object_db::<D>(key, &borrow_db_checked())
}

/// Try to get from in-memory cache first, then fall back to disk
fn get_from_memory_cache(key: &str) -> Option<Vec<u8>> {
    let cache = MEMORY_CACHE.lock().ok()?;
    if let Some(entry) = cache.get(key) {
        // Check if entry has expired
        if entry.expiry >= get_sys_time_in_secs() {
            return Some(entry.data.clone());
        }
    }
    None
}

/// Store in both memory and disk (write-through)
fn store_in_memory_cache(key: String, data: Vec<u8>, expiry: u64) {
    if let Ok(mut cache) = MEMORY_CACHE.lock() {
        // Simple eviction: if cache is full, clear oldest entries
        if cache.len() >= MAX_MEMORY_CACHE_SIZE {
            // Remove all expired entries first
            cache.retain(|_, entry| entry.expiry >= get_sys_time_in_secs());
            // If still too full, clear 25% of entries
            if cache.len() >= MAX_MEMORY_CACHE_SIZE {
                let to_remove = (MAX_MEMORY_CACHE_SIZE / 4).max(1);
                let keys_to_remove: Vec<String> = cache.keys().take(to_remove).cloned().collect();
                for key in keys_to_remove {
                    cache.remove(&key);
                }
            }
        }
        cache.insert(key, MemoryCacheEntry { data, expiry });
    }
}

pub fn get_cached_object_db<D: DecodeOwned>(
    key: &str,
    db: &Database,
) -> Result<D, RemoteAccessError> {
    // Try memory cache first
    if let Some(bytes) = get_from_memory_cache(key) {
        let data = bitcode::decode::<D>(&bytes)
            .map_err(|e| RemoteAccessError::Cache(io::Error::other(e)))?;
        return Ok(data);
    }

    // Fall back to disk cache
    let bytes = read_sync(&db.cache_dir, key).map_err(RemoteAccessError::Cache)?;
    let data =
        bitcode::decode::<D>(&bytes).map_err(|e| RemoteAccessError::Cache(io::Error::other(e)))?;

    // Store in memory cache for future hits
    let expiry = get_sys_time_in_secs() + 60 * 60 * 24; // Default 24 hour expiry
    store_in_memory_cache(key.to_string(), bytes, expiry);

    Ok(data)
}
pub fn clear_cached_object(key: &str) -> Result<(), RemoteAccessError> {
    clear_cached_object_db(key, &borrow_db_checked())
}

/// Clear from both memory and disk
pub fn clear_cached_object_db(key: &str, db: &Database) -> Result<(), RemoteAccessError> {
    // Remove from memory cache
    if let Ok(mut cache) = MEMORY_CACHE.lock() {
        cache.remove(key);
    }

    // Remove from disk
    delete_sync(&db.cache_dir, key).map_err(RemoteAccessError::Cache)?;
    Ok(())
}

#[derive(Encode, Decode)]
pub struct ObjectCache {
    content_type: String,
    body: Vec<u8>,
    expiry: u64,
}

impl ObjectCache {
    pub fn has_expired(&self) -> bool {
        let current = get_sys_time_in_secs();
        self.expiry < current
    }
}

impl TryFrom<Response<Vec<u8>>> for ObjectCache {
    type Error = CacheError;

    fn try_from(value: Response<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(ObjectCache {
            content_type: value
                .headers()
                .get(CONTENT_TYPE)
                .ok_or(CacheError::HeaderNotFound(CONTENT_TYPE))?
                .to_str()
                .map_err(CacheError::ParseError)?
                .to_owned(),
            body: value.body().clone(),
            expiry: get_sys_time_in_secs() + 60 * 60 * 24,
        })
    }
}
impl TryFrom<ObjectCache> for Response<Vec<u8>> {
    type Error = CacheError;
    fn try_from(value: ObjectCache) -> Result<Self, Self::Error> {
        let resp_builder = ResponseBuilder::new().header(CONTENT_TYPE, value.content_type);
        resp_builder
            .body(value.body)
            .map_err(CacheError::ConstructionError)
    }
}
impl TryFrom<&ObjectCache> for Response<Vec<u8>> {
    type Error = CacheError;

    fn try_from(value: &ObjectCache) -> Result<Self, Self::Error> {
        let resp_builder = ResponseBuilder::new().header(CONTENT_TYPE, value.content_type.clone());
        resp_builder
            .body(value.body.clone())
            .map_err(CacheError::ConstructionError)
    }
}

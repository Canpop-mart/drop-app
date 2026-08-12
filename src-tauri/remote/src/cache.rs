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
use http::{
    Response,
    header::{CACHE_CONTROL, CONTENT_TYPE},
    response::Builder as ResponseBuilder,
};
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

/// Hard cap on the in-memory cache. When full, expired entries are dropped
/// first and, if still full, 25% of entries are evicted — so memory use is
/// bounded regardless of how many distinct objects are fetched.
///
/// 100 was too small for a real library — covers + banners + icons run ~3
/// objects per game, so a ~150-game library thrashed the cache and pushed most
/// reads to disk-decode. Raised so a sizeable library's art stays resident.
const MAX_MEMORY_CACHE_SIZE: usize = 600;

/// `Cache-Control` sent on every object response so the WebView caches images in
/// its OWN store and stops re-hitting this protocol (and re-decoding) on every
/// library↔detail navigation. Deliberately shorter than the cache TTL below:
/// this one is the WebView's copy, which we cannot invalidate ourselves, so a
/// day caps how long a replaced image could linger.
pub const OBJECT_CACHE_CONTROL: &str = "max-age=86400";

/// Default time-to-live for a cached entry, in seconds (one year).
///
/// Applied to *both* layers: it is the in-memory entry's expiry and it is
/// serialised into `ObjectCache::expiry`, so the disk copy carries a TTL too
/// (an earlier comment here claimed it did not — see the `expiry` field in
/// `TryFrom<Response<Vec<u8>>>` below).
///
/// A year, not a day, because object ids are content-addressed: the bytes
/// behind an id never change, and the server itself answers
/// `Cache-Control: private, max-age=31536000, immutable`. At 24 hours every
/// object in the library went stale together once a day and the whole grid
/// re-fetched at once, which is the storm this cache exists to prevent.
const DEFAULT_CACHE_TTL_SECS: u64 = 60 * 60 * 24 * 365;

/// How long a stale entry is held after a *failed* refresh before it is
/// considered worth retrying.
///
/// Without this a failed refresh leaves the entry permanently expired, so the
/// next render sees "stale", fires the same doomed request, fails again, and
/// the page re-storms on every single revisit.
const REFRESH_FAILURE_BACKOFF_SECS: u64 = 45;

/// How long a genuine 404 for an object id is remembered.
///
/// Short enough that art uploaded after the miss appears without restarting the
/// app, long enough that a dead id referenced by a hundred rows costs one
/// request instead of a hundred per render.
const MISSING_OBJECT_TTL_SECS: u64 = 300;

/// Bound on the negative cache, so a server handing out bad ids cannot grow it
/// without limit.
const MAX_MISSING_OBJECTS: usize = 512;

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

    // Write to memory cache with the default TTL
    let expiry = get_sys_time_in_secs() + DEFAULT_CACHE_TTL_SECS;
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

/// Object ids the server has answered 404 for, with the time each marker
/// expires.
///
/// Split into its own type, with the clock passed in, so the expiry and
/// eviction rules are testable without touching `SystemTime::now()`.
#[derive(Default)]
struct MissingObjects {
    entries: HashMap<String, u64>,
}

impl MissingObjects {
    fn note(&mut self, key: &str, now: u64) {
        if self.entries.len() >= MAX_MISSING_OBJECTS {
            self.entries.retain(|_, expiry| *expiry > now);
            // Everything still live and the map is full: the ids are churning
            // faster than they expire, so start over rather than grow.
            if self.entries.len() >= MAX_MISSING_OBJECTS {
                self.entries.clear();
            }
        }
        self.entries
            .insert(key.to_owned(), now + MISSING_OBJECT_TTL_SECS);
    }

    fn contains(&self, key: &str, now: u64) -> bool {
        self.entries.get(key).is_some_and(|expiry| *expiry > now)
    }

    fn forget(&mut self, key: &str) {
        self.entries.remove(key);
    }
}

static MISSING_OBJECTS: Lazy<Mutex<MissingObjects>> =
    Lazy::new(|| Mutex::new(MissingObjects::default()));

/// Record that the server said this object does not exist.
pub fn note_object_missing(key: &str) {
    if let Ok(mut missing) = MISSING_OBJECTS.lock() {
        missing.note(key, get_sys_time_in_secs());
    }
}

/// True while a recent 404 for this object is still remembered. Callers should
/// skip the network entirely rather than re-ask.
pub fn object_known_missing(key: &str) -> bool {
    MISSING_OBJECTS
        .lock()
        .is_ok_and(|missing| missing.contains(key, get_sys_time_in_secs()))
}

/// Drop the 404 marker — the object exists after all.
pub fn forget_object_missing(key: &str) {
    if let Ok(mut missing) = MISSING_OBJECTS.lock() {
        missing.forget(key);
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
    let expiry = get_sys_time_in_secs() + DEFAULT_CACHE_TTL_SECS;
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

    /// Hold this stale copy for a short window after a failed refresh instead
    /// of leaving it expired. See `REFRESH_FAILURE_BACKOFF_SECS`.
    pub fn rearm_after_failure(&mut self) {
        self.expiry = get_sys_time_in_secs() + REFRESH_FAILURE_BACKOFF_SECS;
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
            expiry: get_sys_time_in_secs() + DEFAULT_CACHE_TTL_SECS,
        })
    }
}
impl TryFrom<ObjectCache> for Response<Vec<u8>> {
    type Error = CacheError;
    fn try_from(value: ObjectCache) -> Result<Self, Self::Error> {
        let resp_builder = ResponseBuilder::new()
            .header(CONTENT_TYPE, value.content_type)
            .header(CACHE_CONTROL, OBJECT_CACHE_CONTROL);
        resp_builder
            .body(value.body)
            .map_err(CacheError::ConstructionError)
    }
}
impl TryFrom<&ObjectCache> for Response<Vec<u8>> {
    type Error = CacheError;

    fn try_from(value: &ObjectCache) -> Result<Self, Self::Error> {
        let resp_builder = ResponseBuilder::new()
            .header(CONTENT_TYPE, value.content_type.clone())
            .header(CACHE_CONTROL, OBJECT_CACHE_CONTROL);
        resp_builder
            .body(value.body.clone())
            .map_err(CacheError::ConstructionError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_000_000;

    #[test]
    fn a_noted_id_is_missing_until_its_ttl_runs_out() {
        let mut missing = MissingObjects::default();
        missing.note("abc", NOW);

        assert!(missing.contains("abc", NOW));
        assert!(missing.contains("abc", NOW + MISSING_OBJECT_TTL_SECS - 1));
        assert!(!missing.contains("abc", NOW + MISSING_OBJECT_TTL_SECS));
        assert!(!missing.contains("never-noted", NOW));
    }

    #[test]
    fn forgetting_an_id_lets_it_be_requested_again() {
        let mut missing = MissingObjects::default();
        missing.note("abc", NOW);
        missing.forget("abc");

        assert!(!missing.contains("abc", NOW));
    }

    #[test]
    fn the_negative_cache_stays_bounded() {
        let mut missing = MissingObjects::default();
        // Fill it past the cap with entries that are all still live, so the
        // expiry sweep can't reclaim anything and the clear() path has to.
        for i in 0..(MAX_MISSING_OBJECTS * 2) {
            missing.note(&format!("id-{i}"), NOW);
        }

        assert!(missing.entries.len() <= MAX_MISSING_OBJECTS);
    }

    #[test]
    fn expired_entries_are_swept_before_the_cache_is_cleared() {
        let mut missing = MissingObjects::default();
        for i in 0..MAX_MISSING_OBJECTS {
            missing.note(&format!("old-{i}"), NOW);
        }
        // Well past the TTL: the sweep should reclaim every old entry, so the
        // new one lands without wiping anything that mattered.
        let later = NOW + MISSING_OBJECT_TTL_SECS + 1;
        missing.note("fresh", later);

        assert!(missing.contains("fresh", later));
        assert_eq!(missing.entries.len(), 1);
    }

    #[test]
    fn rearming_revives_an_expired_entry_for_the_backoff_window() {
        let mut entry = ObjectCache {
            content_type: "image/png".to_owned(),
            body: vec![1, 2, 3],
            expiry: 0,
        };
        assert!(entry.has_expired());

        entry.rearm_after_failure();

        assert!(!entry.has_expired());
        assert!(entry.expiry <= get_sys_time_in_secs() + REFRESH_FAILURE_BACKOFF_SECS);
    }
}

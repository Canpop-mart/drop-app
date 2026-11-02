use std::{
    fs::{self, create_dir_all},
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use aes::cipher::{KeyIvInit as _, StreamCipher as _};
use chrono::Utc;
use log::{debug, error, info, warn};
use url::Url;

use crate::{
    db::{DATA_ROOT_DIR, DB, KEY_IV},
    error::{DatabaseError, DatabaseResult},
    migrations::{self, VersionedDatabase, VersionedDatabaseRef},
    models::{self, data::Database},
};

type Aes128Ctr64LE = ctr::Ctr64LE<aes::Aes128>;

pub struct DatabaseInterface {
    data: RwLock<models::data::Database>,
    path: PathBuf,
}
impl DatabaseInterface {
    /// Create the data directories and open (or create) `drop.db`.
    ///
    /// Returns an error instead of panicking on failure: a startup fault here
    /// used to kill the app before any window existed ("opens and closes
    /// instantly") with no explanation. The caller ([`Self::initialize`] via app
    /// setup) shows the player what went wrong and exits.
    pub fn set_up_database() -> DatabaseResult<DatabaseInterface> {
        let db_path = DATA_ROOT_DIR.join("drop.db");
        let games_base_dir = DATA_ROOT_DIR.join("games");
        let logs_root_dir = DATA_ROOT_DIR.join("logs");
        let cache_dir = DATA_ROOT_DIR.join("cache");
        let pfx_dir = DATA_ROOT_DIR.join("pfx");

        debug!("creating data directory at {DATA_ROOT_DIR:?}");
        for dir in [
            DATA_ROOT_DIR.as_path(),
            &games_base_dir,
            &logs_root_dir,
            &cache_dir,
            &pfx_dir,
        ] {
            create_dir_all(dir).map_err(|e| {
                DatabaseError::Io(std::io::Error::new(
                    e.kind(),
                    format!("failed to create directory {}: {}", dir.display(), e),
                ))
            })?;
        }

        let exists = fs::exists(db_path.clone()).unwrap_or_else(|e| {
            warn!("Failed to check if {} exists: {}, assuming it does not", db_path.display(), e);
            false
        });

        if exists {
            match DatabaseInterface::open_at_path(&db_path) {
                Ok(Some(db)) => Ok(db),
                Ok(None) => {
                    warn!("Database file disappeared between exists check and open, creating new");
                    let default = Database::new(games_base_dir, None, cache_dir);
                    DatabaseInterface::create_at_path(&db_path, default)
                }
                Err(e) => handle_invalid_database(e, db_path, games_base_dir, cache_dir),
            }
        } else {
            let default = Database::new(games_base_dir, None, cache_dir);
            debug!("Creating database at path {}", db_path.display());
            DatabaseInterface::create_at_path(&db_path, default)
        }
    }

    /// Initialize the global [`crate::db::DB`] exactly once.
    ///
    /// Must be called from app setup before any other database use. On error the
    /// caller must stop startup and show the player what failed: without a
    /// database nothing else can run.
    pub fn initialize() -> DatabaseResult<&'static DatabaseInterface> {
        if let Some(db) = DB.get() {
            return Ok(db);
        }
        let db = Self::set_up_database()?;
        // `set` fails only if another thread initialized concurrently. Setup runs
        // single-threaded before any command, so a lost race just means we use the
        // instance that won it.
        if DB.set(db).is_err() {
            warn!("database was already initialized by another thread; using that instance");
        }
        match DB.get() {
            Some(db) => Ok(db),
            None => Err(DatabaseError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "database initialization state was lost",
            ))),
        }
    }

    /// Get the initialized global database.
    ///
    /// [`Self::initialize`] runs during app setup before any command or request
    /// can execute, and exits on failure, so by the time this is called the
    /// static is guaranteed to be set. The `expect` only fires if a code path
    /// skipped initialization (a programming error), not on player-facing faults.
    pub fn global() -> &'static DatabaseInterface {
        DB.get().expect("database not initialized; initialize() must run before any database use")
    }

    pub fn open_at_path(db_path: &Path) -> DatabaseResult<Option<DatabaseInterface>> {
        if !db_path.exists() {
            return Ok(None);
        };
        let mut database_data = std::fs::read(db_path)?;
        let (key, iv) = *KEY_IV;
        let mut cipher = Aes128Ctr64LE::new(&key.into(), &iv.into());
        cipher.apply_keystream(&mut database_data);

        let database_data = String::from_utf8(database_data)?;

        // Parse into the versioned envelope, then migrate forward to the
        // schema version this build understands. A schema bump therefore
        // never breaks an existing client's DB — see `crate::migrations`.
        let envelope: VersionedDatabase = ron::from_str(&database_data)?;
        let mut database = migrations::migrate_to_latest(envelope)?;
        // One-time seed of the per-install map from the legacy single-install
        // fields (no-op once it has been populated + persisted).
        database.applications.seed_installs_from_legacy();

        Ok(Some(DatabaseInterface {
            data: RwLock::new(database),
            path: db_path.to_path_buf(),
        }))
    }

    pub fn create_at_path(
        db_path: &Path,
        database: Database,
    ) -> DatabaseResult<DatabaseInterface> {
        Self::write_to_path(db_path, &database)?;
        Ok(DatabaseInterface {
            data: RwLock::new(database),
            path: db_path.to_path_buf(),
        })
    }

    /// Serialize and encrypt the database to disk from a reference, avoiding a full clone.
    ///
    /// Always writes the current [`crate::migrations::SCHEMA_VERSION`].
    /// Uses atomic write (temp file + rename) to prevent corruption if the
    /// process is killed mid-write.
    fn write_to_path(db_path: &Path, database: &Database) -> DatabaseResult<()> {
        // Serialise through the versioned envelope (zero-copy via reference)
        // so the on-disk file is always tagged with its schema version.
        let mut database_data =
            ron::to_string(&VersionedDatabaseRef::V1 { database })?.into_bytes();

        let (key, iv) = *KEY_IV;
        let mut cipher = Aes128Ctr64LE::new(&key.into(), &iv.into());
        cipher.apply_keystream(&mut database_data);

        // Write to a temp file in the same directory, then atomically rename.
        // This ensures the database file is never left in a partially-written state.
        let tmp_path = db_path.with_file_name("drop.db.tmp");
        std::fs::write(&tmp_path, database_data)?;
        std::fs::rename(&tmp_path, db_path)?;
        Ok(())
    }

    pub fn database_is_set_up(&self) -> bool {
        !borrow_db_checked().base_url.is_empty()
    }

    pub fn fetch_base_url(&self) -> Url {
        let handle = borrow_db_checked();
        Url::parse(&handle.base_url).unwrap_or_else(|_| {
            // During setup the base_url may be empty or partially typed.
            // Return a safe placeholder instead of panicking so callers
            // receive a network error rather than a crash.
            Url::parse("http://invalid.localhost").expect("hardcoded URL must parse")
        })
    }

    fn borrow_data(
        &self,
    ) -> Result<
        std::sync::RwLockReadGuard<'_, Database>,
        PoisonError<std::sync::RwLockReadGuard<'_, Database>>,
    > {
        self.data.read()
    }

    fn borrow_data_mut(
        &self,
    ) -> Result<
        std::sync::RwLockWriteGuard<'_, Database>,
        PoisonError<std::sync::RwLockWriteGuard<'_, Database>>,
    > {
        self.data.write()
    }
}

/// Recover from a database file that could not be loaded.
///
/// The error is now a typed [`DatabaseError`], so the log distinguishes a
/// corrupt payload from an I/O fault. In every case the unreadable file is
/// renamed aside (never deleted) and a fresh database is created — the
/// `prev_database` pointer lets the UI tell the user where the backup went.
fn handle_invalid_database(
    error: DatabaseError,
    db_path: PathBuf,
    games_base_dir: PathBuf,
    cache_dir: PathBuf,
) -> DatabaseResult<DatabaseInterface> {
    // Only genuinely-corrupt databases get reset. A transient I/O fault (flaky
    // mount, permissions) is NOT corruption — resetting here would rename the real
    // DB aside and wipe the media cache, destroying every install record and
    // cached image over a temporary read error.
    let should_reset = match &error {
        DatabaseError::Io(_) => {
            error!("database could not be read due to an I/O error: {error}");
            false
        }
        DatabaseError::InvalidUtf8(_) | DatabaseError::Deserialize(_) => {
            warn!("database file is corrupt, backing it up and starting fresh: {error}");
            true
        }
        DatabaseError::UnsupportedVersion { .. } => {
            warn!("database schema is unsupported by this build: {error}");
            true
        }
        DatabaseError::MigrationFailed { .. } => {
            warn!("database migration failed, backing up the original: {error}");
            true
        }
        // Serialize errors only occur while WRITING; reaching this read-recovery
        // path with one is unexpected, so abort rather than wipe.
        DatabaseError::Serialize(_) => {
            error!("unexpected serialize error during database recovery: {error}");
            false
        }
    };

    if !should_reset {
        // Abort startup with the original error; leave the DB + cache untouched.
        return Err(error);
    }

    let new_path = {
        let time = Utc::now().timestamp();
        let mut base = db_path.clone();
        base.set_file_name(format!("drop.db.backup-{time}"));
        base
    };
    info!("old database stored at: {}", new_path.to_string_lossy());
    fs::rename(&db_path, &new_path)?;
    fs::remove_dir_all(cache_dir.clone())?;
    fs::create_dir_all(cache_dir.clone())?;

    let db = Database::new(games_base_dir, Some(new_path), cache_dir);

    DatabaseInterface::create_at_path(&db_path, db)
}

// To automatically save the database upon drop
pub struct DBRead<'a>(RwLockReadGuard<'a, Database>);
pub struct DBWrite<'a>(ManuallyDrop<RwLockWriteGuard<'a, Database>>);
impl<'a> Deref for DBWrite<'a> {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'a> DerefMut for DBWrite<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a> Deref for DBRead<'a> {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Drop for DBWrite<'_> {
    fn drop(&mut self) {
        // Save BEFORE releasing the write lock to prevent concurrent writes.
        // The old code dropped the lock first then called save() which took a
        // read lock — this allowed two concurrent saves to race on the file.
        match DatabaseInterface::write_to_path(&DatabaseInterface::global().path, &self.0) {
            Ok(()) => {}
            Err(e) => {
                error!("database failed to save with error {e}");
                // Don't panic — this would poison the RwLock and make ALL
                // subsequent database access crash the app in a loop.
                // Log the error and continue; data is still in memory.
                error!("WARNING: in-memory database may be ahead of disk");
            }
        }

        // Now release the write lock
        unsafe {
            ManuallyDrop::drop(&mut self.0);
        }
    }
}

pub fn borrow_db_checked<'a>() -> DBRead<'a> {
    // A PoisonError means some earlier thread panicked while holding the
    // write lock. The in-memory state may be inconsistent, but propagating
    // a panic from every reader would brick the app. Log loudly and
    // recover the guard so the rest of the app keeps running.
    let guard = DatabaseInterface::global()
        .borrow_data()
        .unwrap_or_else(|e| {
            error!("database read lock poisoned, recovering: {e}");
            e.into_inner()
        });
    DBRead(guard)
}

pub fn borrow_db_mut_checked<'a>() -> DBWrite<'a> {
    let guard = DatabaseInterface::global()
        .borrow_data_mut()
        .unwrap_or_else(|e| {
            error!("database write lock poisoned, recovering: {e}");
            e.into_inner()
        });
    DBWrite(ManuallyDrop::new(guard))
}

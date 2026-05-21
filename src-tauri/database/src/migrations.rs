//! Versioned database migrations.
//!
//! ## The risk this addresses
//!
//! The database is an AES-encrypted RON blob on disk. Before this module the
//! only versioning was a single-variant tagged enum (`V0_4_0`). That gave
//! *forward* protection — a build that doesn't know a future variant fails
//! `Deserialize` cleanly — but there was **no migration path**: if a field
//! were added or renamed, every existing client's DB would either fail to
//! parse (and get backed-up-and-wiped by `handle_invalid_database`, losing
//! the user's install records) or silently lose data.
//!
//! ## The mechanism
//!
//! Each schema revision is a variant of [`VersionedDatabase`]. Loading:
//!
//!   1. Deserialize the RON into whatever variant it was written as.
//!   2. [`migrate_to_latest`] walks the variant forward one step at a time
//!      until it reaches [`SCHEMA_VERSION`].
//!   3. The migrated [`Database`] is handed to the rest of the app.
//!
//! Saving always writes [`SCHEMA_VERSION`].
//!
//! ## Adding a new schema version (worked example)
//!
//! Suppose v2 adds a field. The steps are:
//!
//!   1. In `models.rs`, add a `mod v2` with the new shape (copy `v1`, change
//!      what differs). Point the public `pub type Database = v2::Database;`
//!      aliases at it.
//!   2. Here, add `V2 { database: v2::Database }` to [`VersionedDatabase`].
//!   3. Bump [`SCHEMA_VERSION`] to `2`.
//!   4. Extend [`migrate_to_latest`]'s match so `V1` maps its fields into a
//!      `v2::Database` and returns `V2`.
//!
//! Existing v1 databases then migrate transparently on first launch; brand
//! new databases are born at v2. **Never** mutate an old `vN` module in a
//! way that changes its on-disk representation — that breaks the very
//! clients this mechanism exists to protect.

use log::{info, warn};
use serde::{Deserialize, Serialize};

use crate::error::DatabaseError;
use crate::models::data::Database;

/// Newest schema version this build understands and writes.
///
/// Bump this whenever a new [`VersionedDatabase`] variant is added.
pub const SCHEMA_VERSION: u32 = 1;

/// The on-disk envelope: a database tagged with the schema revision that
/// produced it. RON serialises this as e.g. `V1(database: (...))`.
///
/// A future build that doesn't know a newer variant fails to deserialize
/// here — caught and reported as [`DatabaseError::Deserialize`] rather than
/// silently misreading the file.
#[derive(Serialize, Deserialize)]
pub enum VersionedDatabase {
    /// Schema v1 — the shape shipped through the 0.4.x line.
    V1 { database: Database },
}

/// Borrowing counterpart of [`VersionedDatabase`] so the database can be
/// serialised without a full clone.
#[derive(Serialize)]
pub enum VersionedDatabaseRef<'a> {
    V1 { database: &'a Database },
}

impl VersionedDatabase {
    /// The schema version this envelope represents.
    pub fn version(&self) -> u32 {
        match self {
            VersionedDatabase::V1 { .. } => 1,
        }
    }
}

/// Migrate a freshly-deserialised envelope forward to [`SCHEMA_VERSION`].
///
/// Today there is exactly one version, so this validates the version and
/// unwraps the database. The structure is the extension point: when `V2` is
/// added, the `match` below gains a `V1 { database } => migrate_v1_to_v2(..)`
/// arm and the migrated result is fed back through this function (recursing
/// one step) until it reaches the latest variant. Each step is a small,
/// independently-testable function — see the module docs for the worked
/// example.
pub fn migrate_to_latest(envelope: VersionedDatabase) -> Result<Database, DatabaseError> {
    let start = envelope.version();

    if start > SCHEMA_VERSION {
        // Should be unreachable: a newer variant would have failed to
        // deserialize. Guard anyway so a hand-edited / downgraded file is
        // rejected loudly instead of being silently misread.
        warn!(
            "[db-migration] database schema v{start} is newer than supported v{SCHEMA_VERSION}"
        );
        return Err(DatabaseError::UnsupportedVersion {
            found: start.to_string(),
            supported: SCHEMA_VERSION.to_string(),
        });
    }

    if start < SCHEMA_VERSION {
        info!("[db-migration] migrating database from schema v{start} to v{SCHEMA_VERSION}");
    }

    match envelope {
        // Latest version — nothing to migrate.
        VersionedDatabase::V1 { database } => {
            info!(
                "[db-migration] database at schema v{} (latest), no migration needed",
                SCHEMA_VERSION
            );
            Ok(database)
        }
        // When V2 lands, the V1 arm above changes to:
        //   VersionedDatabase::V1 { database } =>
        //       migrate_to_latest(VersionedDatabase::V2 { database: v1_to_v2(database) }),
        // and a new V2 arm returns `Ok(database)`.
    }
}

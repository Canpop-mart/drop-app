//! One error type for the `database` crate.
//!
//! Before this, `interface.rs` threaded `anyhow::Error` everywhere, which
//! flattened "the file is missing", "the bytes are not valid UTF-8" and "the
//! RON failed to parse" into a single opaque value. The recovery path
//! (`handle_invalid_database`) even carried a `// TODO: Make the error
//! relevant rather than just assume that it's a Deserialize error`.
//!
//! [`DatabaseError`] keeps the variants distinct so callers (and logs) can
//! tell an I/O fault from a corrupt-payload fault — corruption warrants
//! backing the file up and starting fresh, an I/O fault generally does not.

use std::fmt::{self, Display};

/// Everything that can go wrong loading, saving or migrating the database.
#[derive(Debug)]
pub enum DatabaseError {
    /// Filesystem I/O failed (read, write, rename, ...).
    Io(std::io::Error),
    /// Decrypted bytes were not valid UTF-8 — almost always corruption or a
    /// wrong encryption key.
    InvalidUtf8(std::string::FromUtf8Error),
    /// The RON payload could not be parsed into the versioned envelope.
    Deserialize(ron::error::SpannedError),
    /// The database could not be serialised back to RON.
    Serialize(ron::Error),
    /// The on-disk schema version is newer than this build understands, or
    /// no migration step exists to bring it forward.
    UnsupportedVersion {
        /// Version found on disk.
        found: String,
        /// Newest version this build can produce.
        supported: String,
    },
    /// A migration step ran but left the database in an invalid shape.
    MigrationFailed {
        /// Version the step was migrating from.
        from: String,
        /// Version the step was migrating to.
        to: String,
        /// Human-readable cause.
        reason: String,
    },
}

impl Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::Io(e) => write!(f, "database I/O error: {e}"),
            DatabaseError::InvalidUtf8(e) => {
                write!(f, "database file is not valid UTF-8 (corrupt or wrong key): {e}")
            }
            DatabaseError::Deserialize(e) => write!(f, "could not parse database: {e}"),
            DatabaseError::Serialize(e) => write!(f, "could not serialize database: {e}"),
            DatabaseError::UnsupportedVersion { found, supported } => write!(
                f,
                "database schema version {found} is not supported by this build \
                 (newest understood: {supported})"
            ),
            DatabaseError::MigrationFailed { from, to, reason } => {
                write!(f, "migration {from} -> {to} failed: {reason}")
            }
        }
    }
}

impl std::error::Error for DatabaseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DatabaseError::Io(e) => Some(e),
            DatabaseError::InvalidUtf8(e) => Some(e),
            DatabaseError::Deserialize(e) => Some(e),
            DatabaseError::Serialize(e) => Some(e),
            DatabaseError::UnsupportedVersion { .. } | DatabaseError::MigrationFailed { .. } => {
                None
            }
        }
    }
}

impl From<std::io::Error> for DatabaseError {
    fn from(e: std::io::Error) -> Self {
        DatabaseError::Io(e)
    }
}

impl From<std::string::FromUtf8Error> for DatabaseError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        DatabaseError::InvalidUtf8(e)
    }
}

impl From<ron::error::SpannedError> for DatabaseError {
    fn from(e: ron::error::SpannedError) -> Self {
        DatabaseError::Deserialize(e)
    }
}

impl From<ron::Error> for DatabaseError {
    fn from(e: ron::Error) -> Self {
        DatabaseError::Serialize(e)
    }
}

/// Convenience alias for crate-internal results.
pub type DatabaseResult<T> = Result<T, DatabaseError>;

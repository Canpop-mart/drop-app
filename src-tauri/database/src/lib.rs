#![feature(nonpoison_rwlock)]

pub mod db;
pub mod debug;
pub mod error;
pub mod interface;
pub mod migrations;
pub mod models;
pub mod platform;

pub use db::DB;
pub use error::{DatabaseError, DatabaseResult};
pub use interface::{borrow_db_checked, borrow_db_mut_checked};
pub use migrations::SCHEMA_VERSION;
pub use models::data::{
    ApplicationTransientStatus, Database, DatabaseApplications, DatabaseAuth, DownloadType,
    DownloadableMetadata, GameDownloadStatus, GameVersion, PendingQueueEntry, Settings,
};

#![feature(slice_concat_trait)]
#![feature(sync_nonpoison)]
#![feature(nonpoison_mutex)]

pub mod achievements;
pub mod auth;
#[macro_use]
pub mod cache;
pub mod error;
pub mod fetch_object;
pub mod goldberg;
pub mod playtime;
pub mod retroarch;
pub mod requests;
pub mod save_sync;
pub mod server_proto;
pub mod streaming_sessions;
pub mod utils;

pub use auth::setup;

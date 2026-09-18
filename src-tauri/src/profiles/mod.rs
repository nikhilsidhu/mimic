//! What mimic keeps on disk: profiles, per-champion overlays, the account map and
//! pre-apply snapshots. Everything is plain JSON so a profile file is also its export.

mod model;
mod store;

pub use model::{Account, Accounts, Overlay, Profile, Snapshot, SCHEMA_VERSION};
pub use store::Store;

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("{path}: {source}")]
    Io { path: std::path::PathBuf, source: std::io::Error },
    #[error("{path} is not valid: {source}")]
    Json { path: std::path::PathBuf, source: serde_json::Error },
    #[error("{path} was written by a newer mimic (schema {found}, this build reads up to {SCHEMA_VERSION})")]
    UnsupportedSchema { path: std::path::PathBuf, found: u32 },
    #[error("{0:?} is not a valid profile id")]
    InvalidId(String),
}

pub type Result<T> = std::result::Result<T, ProfileError>;

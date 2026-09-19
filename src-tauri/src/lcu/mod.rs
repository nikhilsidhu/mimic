//! Talking to the League client (LCU): lockfile discovery and the local REST API.
//!
//! Only the League client is touched, never the game process.

mod client;
mod events;
mod lockfile;

pub use client::{LcuClient, Summoner};
pub use events::{subscribe, LcuEvent};
pub use lockfile::Lockfile;

#[derive(Debug, thiserror::Error)]
pub enum LcuError {
    #[error("malformed lockfile: {0}")]
    MalformedLockfile(String),
    #[error("could not read lockfile: {0}")]
    Io(#[from] std::io::Error),
    #[error("request to the League client failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("WebSocket to the League client failed: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("could not set up TLS: {0}")]
    Tls(#[from] native_tls::Error),
    #[error("League client answered {status} for {path}: {body}")]
    Status { path: String, status: u16, body: String },
    #[error("settings file {0:?} has no LCU endpoint")]
    UnknownSettingsFile(String),
}

pub type Result<T> = std::result::Result<T, LcuError>;

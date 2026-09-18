//! Model of League's `PersistedSettings.json` and the operations profiles are built on.
//!
//! The file on disk is the canonical, full shape: every value is a string. The LCU
//! endpoints expose only a typed subset of it, so conversion to and from the LCU view
//! lives in a separate layer and never leaks into this module.

mod model;
mod ops;

pub use model::{PersistedFile, PersistedSection, PersistedSetting, PersistedSettings};
pub use ops::{diff, merge, overlay_between, Change, SettingsMap};

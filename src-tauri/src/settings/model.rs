use serde::{Deserialize, Serialize};

/// `PersistedSettings.json` exactly as League writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub files: Vec<PersistedFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedFile {
    pub name: String,
    pub sections: Vec<PersistedSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedSection {
    pub name: String,
    pub settings: Vec<PersistedSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedSetting {
    pub name: String,
    pub value: String,
}

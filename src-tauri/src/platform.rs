//! Where things live on this machine: the League install and mimic's own data.

use std::path::{Path, PathBuf};

const DEFAULT_INSTALL: &str = r"C:\Riot Games\League of Legends";

/// A League of Legends install directory (the live patchline, never PBE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeagueInstall {
    root: PathBuf,
}

impl LeagueInstall {
    /// Accepts `root` only if it looks like a League install. Used for the manual picker.
    pub fn at(root: impl Into<PathBuf>) -> Option<Self> {
        let root = root.into();
        root.join("LeagueClient.exe").is_file().then_some(LeagueInstall { root })
    }

    /// Finds the install from Riot's own metadata, falling back to the default path.
    pub fn detect() -> Option<Self> {
        let program_data = PathBuf::from(std::env::var_os("ProgramData")?).join("Riot Games");
        let read = |relative: &str| std::fs::read_to_string(program_data.join(relative)).unwrap_or_default();

        let product_settings = read(r"Metadata\league_of_legends.live\league_of_legends.live.product_settings.yaml");
        let client_installs = read("RiotClientInstalls.json");

        install_from_product_settings(&product_settings)
            .into_iter()
            .chain(installs_from_riot_client_installs(&client_installs))
            .chain([PathBuf::from(DEFAULT_INSTALL)])
            .find_map(LeagueInstall::at)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn persisted_settings(&self) -> PathBuf {
        self.root.join("Config").join("PersistedSettings.json")
    }
}

/// mimic's data directory, `%APPDATA%\mimic`.
pub fn data_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("mimic"))
}

/// Reads `product_install_full_path: "C:/Riot Games/League of Legends"`.
fn install_from_product_settings(yaml: &str) -> Option<PathBuf> {
    let value = yaml.lines().find_map(|line| line.trim().strip_prefix("product_install_full_path:"))?;
    let path = value.trim().trim_matches('"');
    (!path.is_empty()).then(|| PathBuf::from(path))
}

/// The install directories listed under `associated_client`, which also holds PBE.
fn installs_from_riot_client_installs(json: &str) -> Vec<PathBuf> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(clients) = value.get("associated_client").and_then(|clients| clients.as_object()) else {
        return Vec::new();
    };
    clients.keys().filter(|path| !path.contains("(PBE)")).map(PathBuf::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_install_path_from_product_settings() {
        let yaml = "locale_data:\n    default_locale: \"en_US\"\nproduct_install_full_path: \"D:/Games/League of Legends\"\nproduct_install_root: \"D:/Games/\"\n";
        assert_eq!(install_from_product_settings(yaml), Some(PathBuf::from("D:/Games/League of Legends")));
        assert_eq!(install_from_product_settings(""), None);
        assert_eq!(install_from_product_settings("product_install_full_path: \"\""), None);
    }

    #[test]
    fn reads_live_installs_from_riot_client_installs() {
        let json = r#"{
            "associated_client": {
                "C:/Riot Games/League of Legends (PBE)/": "C:/Riot Games/Riot Client/RiotClientServices.exe",
                "C:/Riot Games/League of Legends/": "C:/Riot Games/Riot Client/RiotClientServices.exe"
            },
            "rc_default": "C:/Riot Games/Riot Client/RiotClientServices.exe"
        }"#;
        assert_eq!(installs_from_riot_client_installs(json), [PathBuf::from("C:/Riot Games/League of Legends/")]);
        assert!(installs_from_riot_client_installs("not json").is_empty());
        assert!(installs_from_riot_client_installs("{}").is_empty());
    }

    #[test]
    fn rejects_directories_that_are_not_an_install() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(LeagueInstall::at(dir.path()), None);

        std::fs::write(dir.path().join("LeagueClient.exe"), b"").unwrap();
        let install = LeagueInstall::at(dir.path()).unwrap();
        assert_eq!(install.persisted_settings(), dir.path().join("Config").join("PersistedSettings.json"));
    }

    /// Runs against this machine's real install when there is one.
    #[test]
    fn detects_a_real_install_when_present() {
        if let Some(install) = LeagueInstall::detect() {
            assert!(install.root().join("LeagueClient.exe").is_file());
            assert!(!install.root().to_string_lossy().contains("(PBE)"));
        }
    }
}

//! Moving profiles between machines: a profile's file is its export format.

use super::actions::{ActionError, Result};
use super::Engine;
use crate::profiles::{Profile, SCHEMA_VERSION};

/// The settings files a profile may carry.
const FILES: [&str; 2] = ["Game.cfg", "Input.ini"];

impl Engine {
    /// A profile as the file to write, with a file name to suggest.
    pub fn export_profile(&self, id: &str) -> Result<(String, Vec<u8>)> {
        let profile = self.inner.store.load_profile(id)?.ok_or_else(|| ActionError::NoSuchProfile(id.to_owned()))?;
        let json = serde_json::to_vec_pretty(&profile).map_err(|err| ActionError::InvalidImport(err.to_string()))?;
        Ok((format!("{}.mimic.json", file_stem(&profile.name)), json))
    }

    /// Adds the profile in an exported file. It gets a fresh id, so it never replaces
    /// anything, and a name that is not taken yet.
    pub async fn import_profile(&self, bytes: &[u8]) -> Result<Profile> {
        let _guard = self.inner.action_lock.lock().await;
        let taken: Vec<String> = self.inner.store.list_profiles()?.into_iter().map(|profile| profile.name).collect();
        let profile = imported(bytes, &taken)?;
        self.inner.store.save_profile(&profile)?;
        tracing::info!(id = %profile.id, name = %profile.name, settings = profile.settings.len(), "imported profile");
        self.inner.changed.send_modify(|revision| *revision += 1);
        Ok(profile)
    }
}

/// Reads an exported profile and makes it safe to keep: only known settings files, a
/// fresh id (the one in the file would end up in a file name), and a free name.
fn imported(bytes: &[u8], taken: &[String]) -> Result<Profile> {
    let invalid = |why: &str| ActionError::InvalidImport(why.to_owned());
    let file: Profile = serde_json::from_slice(bytes).map_err(|_| invalid("it is not a mimic profile"))?;
    if file.schema > SCHEMA_VERSION {
        return Err(invalid("it was made by a newer mimic"));
    }
    if file.settings.is_empty() {
        return Err(invalid("it holds no settings"));
    }
    if let Some(unknown) = file.settings.0.keys().find(|name| !FILES.contains(&name.as_str())) {
        return Err(ActionError::InvalidImport(format!("it has settings for {unknown:?}, which mimic does not know")));
    }

    let name = match file.name.trim() {
        "" => "Imported",
        name => name,
    };
    let name: String = name.chars().take(40).collect();
    let name = (1..)
        .map(|n| if n == 1 { name.clone() } else { format!("{name} {n}") })
        .find(|candidate| !taken.contains(candidate))
        .expect("an unbounded range always yields a free name");

    let mut profile = Profile::new(&name, file.settings);
    profile.created = file.created;
    profile.client = file.client;
    Ok(profile)
}

/// A file name made from a profile name: what Windows allows, nothing else.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' })
        .collect();
    match stem.trim() {
        "" => "profile".to_owned(),
        stem => stem.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SettingsMap;

    fn exported(name: &str, id: &str) -> Vec<u8> {
        let mut settings = SettingsMap::default();
        settings.set("Input.ini", "GameEvents", "evtCastSpell1", "[q]");
        let profile = Profile { id: id.to_owned(), ..Profile::new(name, settings) };
        serde_json::to_vec(&profile).unwrap()
    }

    #[test]
    fn an_import_gets_its_own_id_and_a_free_name() {
        let profile = imported(&exported("Main", "../../evil"), &["Main".to_owned(), "Main 2".to_owned()]).unwrap();
        assert_eq!(profile.name, "Main 3");
        assert!(profile.id.chars().all(|c| c.is_ascii_hexdigit()), "{}", profile.id);
        assert_eq!(profile.settings.get("Input.ini", "GameEvents", "evtCastSpell1"), Some("[q]"));
    }

    #[test]
    fn refuses_files_that_are_not_profiles() {
        assert!(matches!(imported(b"not json", &[]), Err(ActionError::InvalidImport(_))));
        assert!(matches!(imported(br#"{"some":"json"}"#, &[]), Err(ActionError::InvalidImport(_))));

        let mut odd: serde_json::Value = serde_json::from_slice(&exported("Main", "a")).unwrap();
        odd["settings"]["Other.cfg"] = serde_json::json!({ "S": { "k": "v" } });
        assert!(matches!(imported(odd.to_string().as_bytes(), &[]), Err(ActionError::InvalidImport(_))));

        let mut newer: serde_json::Value = serde_json::from_slice(&exported("Main", "a")).unwrap();
        newer["schema"] = serde_json::json!(SCHEMA_VERSION + 1);
        assert!(matches!(imported(newer.to_string().as_bytes(), &[]), Err(ActionError::InvalidImport(_))));
    }

    #[test]
    fn file_names_are_safe_on_windows() {
        assert_eq!(file_stem("hide under bush"), "hide under bush");
        assert_eq!(file_stem("a/b:c*?"), "a_b_c__");
        assert_eq!(file_stem("   "), "profile");
    }
}

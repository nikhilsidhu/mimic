use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::model::{PersistedFile, PersistedSection, PersistedSetting, PersistedSettings};

/// Keys that describe the machine or the game build rather than a preference.
/// `PersistedSettings.json` is already Riot's portable subset, so this stays small.
const LOCAL_KEYS: &[(&str, &str, &str)] = &[("Game.cfg", "General", "CfgVersion")];

fn is_local(file: &str, section: &str, key: &str) -> bool {
    LOCAL_KEYS.contains(&(file, section, key))
}

/// Whether a key changes without the user changing it, so that a difference in it
/// alone is not something to apply, report or ask about. Such keys still belong in a
/// profile and go along whenever something real is written.
///
/// - Window layout the game rewrites on its own: where the chat, shop and death recap
///   sit and how big the shop is.
/// - The push-to-talk key, which the client's voice settings own: the client sets it
///   back by itself, at login and after games, whatever is written to it.
pub fn is_volatile(file: &str, _section: &str, key: &str) -> bool {
    match file {
        "Game.cfg" => {
            key.contains("NativeOffset")
                || key.starts_with("ItemShopPrev")
                || key.starts_with("ItemShopResize")
                || matches!(key, "ChatX" | "ChatY")
        }
        "Input.ini" => key == "evtPushToTalk",
        _ => false,
    }
}

type Section = BTreeMap<String, String>;
type File = BTreeMap<String, Section>;

/// Portable settings as `file -> section -> key -> value`. This is the shape stored in
/// profiles; an overlay is the same shape holding only the keys it overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SettingsMap(pub BTreeMap<String, File>);

impl SettingsMap {
    pub fn get(&self, file: &str, section: &str, key: &str) -> Option<&str> {
        self.0.get(file)?.get(section)?.get(key).map(String::as_str)
    }

    pub fn set(&mut self, file: &str, section: &str, key: &str, value: &str) {
        self.0
            .entry(file.to_owned())
            .or_default()
            .entry(section.to_owned())
            .or_default()
            .insert(key.to_owned(), value.to_owned());
    }

    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// True when every key in here is volatile layout (see [`is_volatile`]), which
    /// includes being empty.
    pub fn is_only_volatile(&self) -> bool {
        self.iter().all(|(file, section, key, _)| is_volatile(file, section, key))
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Every `(file, section, key, value)` in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str, &str, &str)> {
        self.0.iter().flat_map(|(file, sections)| {
            sections.iter().flat_map(move |(section, settings)| {
                settings
                    .iter()
                    .map(move |(key, value)| (file.as_str(), section.as_str(), key.as_str(), value.as_str()))
            })
        })
    }
}

impl From<&PersistedSettings> for SettingsMap {
    fn from(persisted: &PersistedSettings) -> Self {
        let mut map = SettingsMap::default();
        for file in &persisted.files {
            for section in &file.sections {
                for setting in &section.settings {
                    if !is_local(&file.name, &section.name, &setting.name) {
                        map.set(&file.name, &section.name, &setting.name, &setting.value);
                    }
                }
            }
        }
        map
    }
}

impl From<&SettingsMap> for PersistedSettings {
    fn from(map: &SettingsMap) -> Self {
        let files = map
            .0
            .iter()
            .map(|(name, sections)| PersistedFile {
                name: name.clone(),
                sections: sections
                    .iter()
                    .map(|(name, settings)| PersistedSection {
                        name: name.clone(),
                        settings: settings
                            .iter()
                            .map(|(name, value)| PersistedSetting { name: name.clone(), value: value.clone() })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        PersistedSettings { description: None, files }
    }
}

/// One key that differs between two settings maps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub file: String,
    pub section: String,
    pub key: String,
    /// `None` when the key only exists in `to`.
    pub from: Option<String>,
    /// `None` when the key only exists in `from`.
    pub to: Option<String>,
}

impl Change {
    fn new(file: &str, section: &str, key: &str, from: Option<&str>, to: Option<&str>) -> Self {
        Change {
            file: file.to_owned(),
            section: section.to_owned(),
            key: key.to_owned(),
            from: from.map(str::to_owned),
            to: to.map(str::to_owned),
        }
    }
}

/// Everything that has to change to get from `from` to `to`.
pub fn diff(from: &SettingsMap, to: &SettingsMap) -> Vec<Change> {
    let mut changes = Vec::new();
    for (file, section, key, old) in from.iter() {
        let new = to.get(file, section, key);
        if new != Some(old) {
            changes.push(Change::new(file, section, key, Some(old), new));
        }
    }
    for (file, section, key, new) in to.iter() {
        if from.get(file, section, key).is_none() {
            changes.push(Change::new(file, section, key, None, Some(new)));
        }
    }
    changes.sort_by(|a, b| (&a.file, &a.section, &a.key).cmp(&(&b.file, &b.section, &b.key)));
    changes
}

/// `base ⊕ overlay`: the overlay's keys win, everything else comes from the base.
pub fn merge(base: &SettingsMap, overlay: &SettingsMap) -> SettingsMap {
    let mut merged = base.clone();
    for (file, section, key, value) in overlay.iter() {
        merged.set(file, section, key, value);
    }
    merged
}

/// The overlay that turns `base` into `target`. Keys missing from `target` are ignored:
/// an overlay can only override, never delete.
pub fn overlay_between(base: &SettingsMap, target: &SettingsMap) -> SettingsMap {
    let mut overlay = SettingsMap::default();
    for change in diff(base, target) {
        if let Some(value) = &change.to {
            overlay.set(&change.file, &change.section, &change.key, value);
        }
    }
    overlay
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real `PersistedSettings.json` from a Windows install.
    const FIXTURE: &str = include_str!("../../tests/fixtures/PersistedSettings.json");

    fn persisted() -> PersistedSettings {
        serde_json::from_str(FIXTURE).unwrap()
    }

    fn sample() -> SettingsMap {
        SettingsMap::from(&persisted())
    }

    #[test]
    fn parses_and_drops_local_keys() {
        let persisted = persisted();
        let total: usize = persisted.files.iter().flat_map(|f| &f.sections).map(|s| s.settings.len()).sum();
        let map = sample();

        assert_eq!(map.0.keys().collect::<Vec<_>>(), ["Game.cfg", "Input.ini"]);
        assert_eq!(map.len(), total - LOCAL_KEYS.len());
        assert!(map.get("Input.ini", "GameEvents", "evtCastSpell1").is_some());
        assert!(map.get("Game.cfg", "General", "CameraMode").is_some());
        assert_eq!(map.get("Game.cfg", "General", "CfgVersion"), None);
    }

    #[test]
    fn round_trips_through_persisted_shape() {
        let map = sample();
        let persisted = PersistedSettings::from(&map);
        assert_eq!(SettingsMap::from(&persisted), map);
    }

    #[test]
    fn diff_reports_changed_added_and_removed() {
        let from = sample();
        let cast1 = from.get("Input.ini", "GameEvents", "evtCastSpell1").unwrap();
        let palette = from.get("Game.cfg", "ColorPalette", "ColorPalette").unwrap();

        let mut to = from.clone();
        to.set("Input.ini", "GameEvents", "evtCastSpell1", "[Shift][F12]");
        to.set("Input.ini", "GameEvents", "evtNotARealEvent", "[e]");
        to.0.get_mut("Game.cfg").unwrap().remove("ColorPalette");

        let changes = diff(&from, &to);
        assert_eq!(
            changes,
            vec![
                Change::new("Game.cfg", "ColorPalette", "ColorPalette", Some(palette), None),
                Change::new("Input.ini", "GameEvents", "evtCastSpell1", Some(cast1), Some("[Shift][F12]")),
                Change::new("Input.ini", "GameEvents", "evtNotARealEvent", None, Some("[e]")),
            ]
        );
        assert!(diff(&from, &from).is_empty());
    }

    #[test]
    fn layout_the_game_rewrites_is_volatile() {
        for key in ["DeathRecapNativeOffsetX", "ItemShopPrevX", "ItemShopResizeWidth"] {
            assert!(is_volatile("Game.cfg", "HUD", key), "{key}");
        }
        assert!(is_volatile("Game.cfg", "Chat", "ChatX"));
        assert!(is_volatile("Game.cfg", "ItemShop", "NativeOffsetY"));
        assert!(is_volatile("Input.ini", "GameEvents", "evtPushToTalk"));
        assert!(!is_volatile("Game.cfg", "HUD", "MinimapScale"));
        assert!(!is_volatile("Input.ini", "GameEvents", "evtCastSpell1"));

        // The real file has such keys, and a diff made only of them counts as nothing.
        let base = sample();
        let mut moved = base.clone();
        moved.set("Game.cfg", "HUD", "DeathRecapNativeOffsetX", "0.0852");
        assert!(overlay_between(&base, &moved).is_only_volatile());
        moved.set("Input.ini", "GameEvents", "evtCastSpell1", "[Shift][F12]");
        assert!(!overlay_between(&base, &moved).is_only_volatile());
        assert!(SettingsMap::default().is_only_volatile());
    }

    #[test]
    fn overlay_merges_back_to_target() {
        let base = sample();
        let mut target = base.clone();
        target.set("Input.ini", "GameEvents", "evtCastSpell2", "[Shift][F11]");
        target.set("Game.cfg", "General", "CameraMode", "not-a-real-mode");

        let overlay = overlay_between(&base, &target);
        assert_eq!(overlay.len(), 2);
        assert_eq!(merge(&base, &overlay), target);
        assert_eq!(merge(&base, &SettingsMap::default()), base);
    }
}

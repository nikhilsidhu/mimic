//! Champion names and icons, taken from the user's own League client and cached on
//! disk so they are there when the client is not. Nothing of Riot's ships with mimic.
//!
//! ```text
//! cache/champions.json
//! cache/champion-icons/<id>.png
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::lcu::LcuClient;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Champion {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Champions {
    dir: PathBuf,
}

impl Champions {
    /// `data_dir` is mimic's data directory.
    pub fn new(data_dir: &Path) -> Self {
        Champions { dir: data_dir.join("cache") }
    }

    /// The cached champions, sorted by name. Empty until a client has been seen once.
    pub fn list(&self) -> Vec<Champion> {
        let Ok(bytes) = std::fs::read(self.dir.join("champions.json")) else { return Vec::new() };
        serde_json::from_slice(&bytes).unwrap_or_default()
    }

    pub fn name(&self, id: u32) -> Option<String> {
        self.list().into_iter().find(|champion| champion.id == id).map(|champion| champion.name)
    }

    pub fn icon_path(&self, id: u32) -> PathBuf {
        self.dir.join("champion-icons").join(format!("{id}.png"))
    }

    /// Brings the cache up to date with the connected client: the list every time, as
    /// it is one small request, and only the icons that are missing. Returns how many
    /// icons were fetched.
    pub async fn refresh(&self, client: &LcuClient) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let summary: Vec<Summary> = client.get("/lol-game-data/assets/v1/champion-summary.json").await?;
        let mut champions = champions_from(summary);
        champions.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        let icons = self.dir.join("champion-icons");
        std::fs::create_dir_all(&icons)?;
        std::fs::write(self.dir.join("champions.json"), serde_json::to_vec(&champions)?)?;

        let mut fetched = 0;
        for champion in &champions {
            let path = self.icon_path(champion.id);
            if path.exists() {
                continue;
            }
            // One missing icon is not worth losing the rest over.
            match client.get_bytes(&format!("/lol-game-data/assets/v1/champion-icons/{}.png", champion.id)).await {
                Ok(bytes) => {
                    std::fs::write(&path, bytes)?;
                    fetched += 1;
                }
                Err(err) => tracing::warn!(champion = champion.id, "could not fetch icon: {err}"),
            }
        }
        Ok(fetched)
    }
}

/// An entry of the client's `champion-summary.json`.
#[derive(Debug, Deserialize)]
struct Summary {
    id: i64,
    name: String,
}

/// The list has a placeholder "None" with id -1, which is not a champion.
fn champions_from(summary: Vec<Summary>) -> Vec<Champion> {
    summary
        .into_iter()
        .filter_map(|entry| Some(Champion { id: u32::try_from(entry.id).ok().filter(|id| *id > 0)?, name: entry.name }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_the_placeholder_entry() {
        let summary: Vec<Summary> = serde_json::from_str(
            r#"[{"id":-1,"name":"None","alias":"None"},
                {"id":157,"name":"Yasuo","alias":"Yasuo","roles":["fighter"]},
                {"id":18,"name":"Tristana","alias":"Tristana"}]"#,
        )
        .unwrap();
        let champions = champions_from(summary);
        assert_eq!(champions.iter().map(|champion| champion.id).collect::<Vec<_>>(), [157, 18]);
    }

    #[test]
    fn an_empty_cache_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let champions = Champions::new(dir.path());
        assert!(champions.list().is_empty());
        assert_eq!(champions.name(157), None);
        assert!(champions.icon_path(157).ends_with("cache/champion-icons/157.png"));
    }
}

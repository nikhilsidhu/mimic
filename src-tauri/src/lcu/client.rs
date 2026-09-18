use std::time::Duration;

use reqwest::{Method, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{LcuError, Lockfile, Result};
use crate::settings::SettingsMap;

/// The logged-in account, from `/lol-summoner/v1/current-summoner`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summoner {
    pub puuid: String,
    pub game_name: String,
    pub tag_line: String,
}

/// REST client for one running League client instance.
#[derive(Debug, Clone)]
pub struct LcuClient {
    http: reqwest::Client,
    base_url: String,
    password: String,
}

impl LcuClient {
    pub fn new(lockfile: &Lockfile) -> Result<Self> {
        let http = reqwest::Client::builder()
            // The LCU serves a self-signed certificate on loopback.
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(LcuClient { http, base_url: lockfile.base_url(), password: lockfile.password.clone() })
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}{}", self.base_url, path))
            .basic_auth("riot", Some(&self.password))
    }

    async fn send<T: DeserializeOwned>(&self, path: &str, request: RequestBuilder) -> Result<T> {
        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LcuError::Status { path: path.to_owned(), status: status.as_u16(), body });
        }
        Ok(response.json().await?)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.send(path, self.request(Method::GET, path)).await
    }

    pub async fn patch<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        self.send(path, self.request(Method::PATCH, path).json(body)).await
    }

    /// Whether a client is actually answering. A lockfile can outlive its client.
    pub async fn is_alive(&self) -> bool {
        self.gameflow_phase().await.is_ok()
    }

    pub async fn gameflow_phase(&self) -> Result<String> {
        self.get("/lol-gameflow/v1/gameflow-phase").await
    }

    pub async fn current_summoner(&self) -> Result<Summoner> {
        self.get("/lol-summoner/v1/current-summoner").await
    }

    /// True when Riot reset this account's settings, typically after a patch.
    pub async fn did_reset(&self) -> Result<bool> {
        self.get("/lol-game-settings/v1/didreset").await
    }

    /// Pushes the current settings to Riot's servers for the logged-in account.
    pub async fn save_settings(&self) -> Result<bool> {
        self.send("/lol-game-settings/v1/save", self.request(Method::POST, "/lol-game-settings/v1/save")).await
    }

    /// Writes `settings` into the client and saves them to the account.
    ///
    /// The PATCH endpoints accept the file's string values as they are, including keys
    /// and sections that their own GET view leaves out, and rewrite
    /// `PersistedSettings.json`, `game.cfg` and `input.ini` immediately.
    pub async fn apply_settings(&self, settings: &SettingsMap) -> Result<()> {
        for (file, sections) in &settings.0 {
            let path = settings_endpoint(file)?;
            let _: serde_json::Value = self.patch(path, sections).await?;
        }
        self.save_settings().await?;
        Ok(())
    }
}

fn settings_endpoint(file: &str) -> Result<&'static str> {
    match file {
        "Game.cfg" => Ok("/lol-game-settings/v1/game-settings"),
        "Input.ini" => Ok("/lol-game-settings/v1/input-settings"),
        other => Err(LcuError::UnknownSettingsFile(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_settings_files_to_endpoints() {
        assert_eq!(settings_endpoint("Game.cfg").unwrap(), "/lol-game-settings/v1/game-settings");
        assert_eq!(settings_endpoint("Input.ini").unwrap(), "/lol-game-settings/v1/input-settings");
        assert!(matches!(settings_endpoint("Other.cfg"), Err(LcuError::UnknownSettingsFile(_))));
    }

    #[test]
    fn parses_current_summoner() {
        let json = r#"{"accountId":1,"gameName":"Player","tagLine":"NA1","puuid":"abc-123","summonerLevel":20}"#;
        let summoner: Summoner = serde_json::from_str(json).unwrap();
        assert_eq!(summoner, Summoner { puuid: "abc-123".into(), game_name: "Player".into(), tag_line: "NA1".into() });
    }
}

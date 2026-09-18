use std::io::ErrorKind;
use std::path::Path;

use super::{LcuError, Result};

/// Connection details the League client writes to `<install>/lockfile` while it runs:
/// `LeagueClient:<pid>:<port>:<password>:<protocol>`.
///
/// The file is removed on a clean exit but survives a crash or a PC shutdown, so its
/// existence alone does not mean the client is up; see [`super::LcuClient::is_alive`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lockfile {
    pub pid: u32,
    pub port: u16,
    pub password: String,
    pub protocol: String,
}

impl Lockfile {
    pub fn parse(contents: &str) -> Result<Self> {
        let malformed = || LcuError::MalformedLockfile(format!("{} fields", contents.split(':').count()));
        let fields: Vec<&str> = contents.trim().split(':').collect();
        let [_process, pid, port, password, protocol] = fields[..] else {
            return Err(malformed());
        };
        Ok(Lockfile {
            pid: pid.parse().map_err(|_| malformed())?,
            port: port.parse().map_err(|_| malformed())?,
            password: password.to_owned(),
            protocol: protocol.to_owned(),
        })
    }

    /// Reads `<install_dir>/lockfile`. `Ok(None)` means the client is not running.
    pub fn read(install_dir: &Path) -> Result<Option<Self>> {
        match std::fs::read_to_string(install_dir.join("lockfile")) {
            Ok(contents) => Self::parse(&contents).map(Some),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn base_url(&self) -> String {
        format!("{}://127.0.0.1:{}", self.protocol, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_lockfile() {
        let lockfile = Lockfile::parse("LeagueClient:18956:65040:s3cr3t-pass_word:https").unwrap();
        assert_eq!(lockfile.pid, 18956);
        assert_eq!(lockfile.port, 65040);
        assert_eq!(lockfile.password, "s3cr3t-pass_word");
        assert_eq!(lockfile.base_url(), "https://127.0.0.1:65040");
    }

    #[test]
    fn rejects_malformed_lockfiles() {
        assert!(Lockfile::parse("").is_err());
        assert!(Lockfile::parse("LeagueClient:1:2:pw").is_err());
        assert!(Lockfile::parse("LeagueClient:pid:65040:pw:https").is_err());
        assert!(Lockfile::parse("LeagueClient:1:99999:pw:https").is_err());
    }

    #[test]
    fn missing_lockfile_means_client_is_down() {
        let dir = std::env::temp_dir().join("mimic-no-such-install");
        assert_eq!(Lockfile::read(&dir).unwrap(), None);
    }
}

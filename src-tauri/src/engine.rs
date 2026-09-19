//! The background task that follows the League client: finds it, waits until it is
//! ready, tracks the account and game phase, and starts over when it goes away.

mod actions;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::watch;
use tokio::time::sleep;

pub use actions::ActionError;

use crate::lcu::{self, LcuClient, Lockfile, Summoner};
use crate::platform::LeagueInstall;
use crate::profiles::Store;

/// What the engine currently knows, for the tray and the UI.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Status {
    #[default]
    Starting,
    /// No League install was found; the user has to point us at one.
    NoInstall,
    ClientDown,
    /// The client is up but no account is ready yet.
    LoggingIn,
    Connected {
        account: Summoner,
        phase: String,
    },
}

impl Status {
    /// `gameName#tagLine` of the connected account.
    pub fn riot_id(&self) -> Option<String> {
        match self {
            Status::Connected { account, .. } => Some(format!("{}#{}", account.game_name, account.tag_line)),
            _ => None,
        }
    }

    /// The gameflow phase, e.g. `Lobby` or `InProgress`.
    pub fn phase(&self) -> Option<&str> {
        match self {
            Status::Connected { phase, .. } => Some(phase),
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Status::Starting => "Starting…".to_owned(),
            Status::NoInstall => "League of Legends not found".to_owned(),
            Status::ClientDown => "League client is not running".to_owned(),
            Status::LoggingIn => "Waiting for login…".to_owned(),
            Status::Connected { .. } => self.riot_id().unwrap_or_default(),
        }
    }
}

/// Handle to the engine: its status, and the actions in [`actions`].
#[derive(Clone)]
pub struct Engine {
    pub status: watch::Receiver<Status>,
    inner: Arc<Inner>,
}

struct Inner {
    store: Store,
    /// Set while an account is logged in and ready.
    connection: Mutex<Option<Connection>>,
    /// Actions read, snapshot and write; two of them must never interleave.
    action_lock: tokio::sync::Mutex<()>,
    /// Bumped whenever profiles or the active profile change, so the tray can redraw.
    changed: watch::Sender<u64>,
}

#[derive(Clone)]
struct Connection {
    client: LcuClient,
    install: LeagueInstall,
    puuid: String,
}

impl Engine {
    /// Starts the engine on the current Tokio runtime.
    pub fn spawn(store: Store) -> Engine {
        let (status, receiver) = watch::channel(Status::default());
        let inner = Arc::new(Inner {
            store,
            connection: Mutex::new(None),
            action_lock: tokio::sync::Mutex::new(()),
            changed: watch::channel(0).0,
        });
        tokio::spawn(run(status, inner.clone()));
        Engine { status: receiver, inner }
    }

    /// Fires whenever profiles or the active profile change.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.inner.changed.subscribe()
    }
}

async fn run(status: watch::Sender<Status>, inner: Arc<Inner>) {
    let install = loop {
        match LeagueInstall::detect() {
            Some(install) => break install,
            None => {
                status.send_replace(Status::NoInstall);
                sleep(Duration::from_secs(10)).await;
            }
        }
    };
    tracing::info!(install = %install.root().display(), "found League install");

    loop {
        status.send_replace(Status::ClientDown);
        let (lockfile, client) = wait_for_client(&install).await;
        tracing::info!(pid = lockfile.pid, port = lockfile.port, "League client is up");
        status.send_replace(Status::LoggingIn);

        let result = follow(&lockfile, &client, &install, &status, &inner).await;
        *inner.connection.lock().unwrap() = None;
        match result {
            Ok(()) => tracing::info!("League client went away"),
            Err(err) => tracing::warn!("lost the League client: {err}"),
        }
        sleep(Duration::from_secs(2)).await;
    }
}

/// Polls until a lockfile exists whose client actually answers; a lockfile can be stale.
async fn wait_for_client(install: &LeagueInstall) -> (Lockfile, LcuClient) {
    loop {
        if let Ok(Some(lockfile)) = Lockfile::read(install.root()) {
            if let Ok(client) = LcuClient::new(&lockfile) {
                if client.is_alive().await {
                    return (lockfile, client);
                }
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
}

/// Follows one client instance until its WebSocket closes.
async fn follow(
    lockfile: &Lockfile,
    client: &LcuClient,
    install: &LeagueInstall,
    status: &watch::Sender<Status>,
    inner: &Inner,
) -> lcu::Result<()> {
    // Subscribed before any state is read so that no change falls in between.
    let mut events = std::pin::pin!(lcu::subscribe(lockfile).await?);

    // Events that arrive while waiting are dropped: the state read below is newer than
    // all of them, and replaying them afterwards would briefly show a stale phase.
    let ready = async {
        while !client.is_ready().await {
            sleep(Duration::from_secs(1)).await;
        }
    };
    tokio::pin!(ready);
    loop {
        tokio::select! {
            _ = &mut ready => break,
            event = events.next() => match event {
                Some(Ok(_)) => {}
                Some(Err(err)) => return Err(err),
                None => return Ok(()),
            },
        }
    }
    let mut account = client.current_summoner().await?;
    let mut phase = client.gameflow_phase().await?;
    // Logs may end up in bug reports, so the account id is cut short.
    tracing::info!(puuid = %account.puuid.chars().take(8).collect::<String>(), %phase, "account ready");
    let connect = |puuid: &str| {
        let connection = Connection { client: client.clone(), install: install.clone(), puuid: puuid.to_owned() };
        *inner.connection.lock().unwrap() = Some(connection);
    };
    connect(&account.puuid);
    status.send_replace(Status::Connected { account: account.clone(), phase: phase.clone() });

    while let Some(event) = events.next().await {
        let event = event?;
        match event.uri.as_str() {
            "/lol-gameflow/v1/gameflow-phase" => {
                if let Some(new) = event.data.as_str() {
                    tracing::debug!(from = %phase, to = %new, "phase changed");
                    phase = new.to_owned();
                }
            }
            "/lol-summoner/v1/current-summoner" => {
                if let Ok(new) = serde_json::from_value::<Summoner>(event.data) {
                    connect(&new.puuid);
                    account = new;
                }
            }
            _ => continue,
        }
        status.send_if_modified(|current| {
            let new = Status::Connected { account: account.clone(), phase: phase.clone() };
            let changed = *current != new;
            *current = new;
            changed
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_say_what_the_tray_should_show() {
        let account = Summoner { puuid: "p".into(), game_name: "Player".into(), tag_line: "NA1".into() };
        assert_eq!(Status::Connected { account, phase: "Lobby".into() }.label(), "Player#NA1");
        assert_eq!(Status::ClientDown.label(), "League client is not running");
    }
}

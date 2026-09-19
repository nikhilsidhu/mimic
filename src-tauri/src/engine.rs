//! The background task that follows the League client: finds it, waits until it is
//! ready, tracks the account and game phase, and starts over when it goes away.

mod actions;
mod overlays;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

pub use actions::{ActionError, Announcement, Applied, ChampionRef, Drift, DriftChoice, Outcome};

use crate::champions::Champions;
use crate::lcu::{self, LcuClient, Lockfile, Summoner};
use crate::platform::LeagueInstall;
use crate::profiles::Store;

/// Phases that follow a game. `TerminatedInError` is how Practice Tool games end.
const GAME_OVER: [&str; 5] = ["PreEndOfGame", "EndOfGame", "TerminatedInError", "Lobby", "None"];

/// Phases that lead up to a game, and the ones the client idles in. Going from the first
/// to the second means the game did not happen.
const BEFORE_GAME: [&str; 3] = ["Matchmaking", "ReadyCheck", "ChampSelect"];
const IDLE: [&str; 2] = ["Lobby", "None"];

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
    champions: Champions,
    /// Set while an account is logged in and ready.
    connection: Mutex<Option<Connection>>,
    /// The champion of the current or most recent game, for saving changes to it.
    last_champion: Mutex<Option<u32>>,
    /// Actions read, snapshot and write; two of them must never interleave.
    action_lock: tokio::sync::Mutex<()>,
    /// Bumped whenever profiles or the active profile change, so the tray can redraw.
    changed: watch::Sender<u64>,
    /// Things the engine did on its own that the user should hear about.
    notices: mpsc::UnboundedSender<Announcement>,
    notices_out: Mutex<Option<mpsc::UnboundedReceiver<Announcement>>>,
}

#[derive(Clone)]
struct Connection {
    client: LcuClient,
    install: LeagueInstall,
    puuid: String,
}

impl Engine {
    /// Starts the engine on the current Tokio runtime.
    pub fn spawn(store: Store, champions: Champions) -> Engine {
        let (status, receiver) = watch::channel(Status::default());
        let (notices, notices_out) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            store,
            champions,
            connection: Mutex::new(None),
            last_champion: Mutex::new(None),
            action_lock: tokio::sync::Mutex::new(()),
            changed: watch::channel(0).0,
            notices,
            notices_out: Mutex::new(Some(notices_out)),
        });
        let engine = Engine { status: receiver, inner };
        tokio::spawn(run(status, engine.clone()));
        engine
    }

    /// Champion names and icons, cached from the client.
    pub fn champions(&self) -> &Champions {
        &self.inner.champions
    }

    /// Fires whenever profiles or the active profile change.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.inner.changed.subscribe()
    }

    /// What the engine wants shown unasked: an auto-apply, or settings that changed.
    /// There is one receiver; the first caller gets it.
    pub fn take_notices(&self) -> Option<mpsc::UnboundedReceiver<Announcement>> {
        self.inner.notices_out.lock().unwrap().take()
    }
}

async fn run(status: watch::Sender<Status>, engine: Engine) {
    let inner = &engine.inner;
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

        let result = follow(&lockfile, &client, &install, &status, &engine).await;
        *inner.connection.lock().unwrap() = None;
        match result {
            Ok(()) => tracing::info!("League client went away"),
            Err(err) => tracing::warn!("lost the League client: {err}"),
        }
        sleep(Duration::from_secs(2)).await;
    }
}

/// Fetches whatever champion names and icons the cache is missing.
async fn refresh_champions(engine: Engine, client: LcuClient) {
    match engine.inner.champions.refresh(&client).await {
        Ok(0) => {}
        Ok(fetched) => {
            tracing::info!(fetched, "cached champion icons");
            engine.inner.changed.send_modify(|revision| *revision += 1);
        }
        Err(err) => tracing::warn!("could not refresh champion data: {err}"),
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
    engine: &Engine,
) -> lcu::Result<()> {
    let inner = &engine.inner;
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
    // In the background: applying takes a second or two and phases must keep flowing.
    tokio::spawn(engine.clone().on_login(account.clone(), phase.clone()));
    tokio::spawn(refresh_champions(engine.clone(), client.clone()));

    // Whether a game has been running since the last check for changed settings.
    let mut played = phase == "InProgress";
    // The local player's pick as last seen, so that only changes are acted on.
    let mut pick: Option<u32> = None;
    while let Some(event) = events.next().await {
        let event = event?;
        match event.uri.as_str() {
            "/lol-gameflow/v1/gameflow-phase" => {
                if let Some(new) = event.data.as_str() {
                    tracing::debug!(from = %phase, to = %new, "phase changed");
                    let before = std::mem::replace(&mut phase, new.to_owned());
                    if phase == "InProgress" {
                        played = true;
                    } else if played && GAME_OVER.contains(&phase.as_str()) {
                        // The game writes its settings as it exits. Once it is over, see
                        // whether the user changed anything in there; that check also
                        // takes a champion's overlay off again.
                        played = false;
                        tokio::spawn(engine.clone().check_drift_after_game());
                    } else if phase == "Matchmaking" {
                        tokio::spawn(engine.clone().on_matchmaking());
                    } else if BEFORE_GAME.contains(&before.as_str()) && IDLE.contains(&phase.as_str()) {
                        // A dodge or a cancelled queue: no game will take the overlay off.
                        tokio::spawn(engine.clone().on_no_game());
                    }
                }
            }
            "/lol-champ-select/v1/session" => {
                let now = if event.event_type == "Delete" { None } else { overlays::picked_champion(&event.data) };
                if let (Some(champion), true) = (now, now != pick) {
                    tokio::spawn(engine.clone().on_champion(champion));
                }
                pick = now;
                continue;
            }            "/lol-summoner/v1/current-summoner" => {
                if let Ok(new) = serde_json::from_value::<Summoner>(event.data) {
                    if new.puuid != account.puuid {
                        connect(&new.puuid);
                        tokio::spawn(engine.clone().on_login(new.clone(), phase.clone()));
                    }
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

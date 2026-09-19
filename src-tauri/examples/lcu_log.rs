//! Test tool: logs the client events, game launches and config-file writes that
//! mimic's engine depends on, so their timing can be measured across real games.
//!
//!     cargo run --example lcu_log                  read-only
//!     cargo run --example lcu_log -- --apply-test  also runs the apply test below
//!
//! Leave it running, play, then Ctrl+C. Output goes to the console and, with extra
//! detail, to `%APPDATA%\mimic\logs\lcu-log-<unix time>.log`.
//!
//! The apply test finds the deadline for per-champion settings. It binds three unused
//! toggle hotkeys at three moments and restores them after the game:
//!
//!     champion locked  Shift+F10  toggles minion health bars
//!     GameStart        Shift+F9   toggles summoner names
//!     InProgress       Shift+F11  toggles all health bars
//!
//! Whichever of them work in that game were applied early enough.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use serde_json::{json, Value};
use mimic_lib::lcu::{self, LcuClient, LcuEvent, Lockfile};
use mimic_lib::platform::{self, LeagueInstall};
use mimic_lib::settings::{diff, PersistedSettings, SettingsMap};
use time::OffsetDateTime;

const GAME_PROCESS: &str = "League of Legends.exe";

/// `(moment, event, key)` for the apply test. All three are unbound by default.
const MARKERS: [(&str, &str, &str); 3] = [
    ("champion locked", "evtToggleMinionHealthBars", "[Shift][F10]"),
    ("GameStart", "evtShowSummonerNames", "[Shift][F9]"),
    ("InProgress", "evtShowHealthBars", "[Shift][F11]"),
];

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<std::fs::File>>,
    uri_counts: Arc<Mutex<BTreeMap<String, u32>>>,
}

impl Log {
    fn write(&self, kind: &str, detail: &str, console: bool) {
        let now = OffsetDateTime::now_utc();
        let line = format!(
            "{:02}:{:02}:{:02}.{:03}Z  {kind:<9} {detail}",
            now.hour(),
            now.minute(),
            now.second(),
            now.millisecond()
        );
        if console {
            println!("{line}");
        }
        let _ = writeln!(self.file.lock().unwrap(), "{line}");
    }

    fn line(&self, kind: &str, detail: &str) {
        self.write(kind, detail, true);
    }

    /// Bulky detail that would drown the console.
    fn file_only(&self, kind: &str, detail: &str) {
        self.write(kind, detail, false);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let apply_test = std::env::args().any(|arg| arg == "--apply-test");
    let install = LeagueInstall::detect().ok_or("League install not found")?;
    let logs = platform::data_dir().ok_or("APPDATA is not set")?.join("logs");
    std::fs::create_dir_all(&logs)?;
    let now = OffsetDateTime::now_utc();
    let path = logs.join(format!("lcu-log-{}.log", now.unix_timestamp()));
    let log = Log { file: Arc::new(Mutex::new(std::fs::File::create(&path)?)), uri_counts: Default::default() };
    log.line("START", &format!("{now} install={} apply_test={apply_test}", install.root().display()));
    log.line("START", &format!("log={}", path.display()));

    let markers = Arc::new(Mutex::new(Markers::default()));

    // Checks the apply test's write path without needing a game: bind all three, restore.
    if std::env::args().any(|arg| arg == "--apply-selftest") {
        let lockfile = Lockfile::read(install.root())?.ok_or("client is not running")?;
        let client = LcuClient::new(&lockfile)?;
        for moment in [Moment::ChampionLocked, Moment::GameStart, Moment::InProgress, Moment::GameOver] {
            handle_moment(&client, &log, &markers, moment).await;
        }
        return Ok(());
    }

    tokio::spawn(watch_files(install.clone(), log.clone()));
    tokio::spawn(watch_game_process(log.clone()));
    tokio::select! {
        _ = follow_client(install.clone(), log.clone(), apply_test, markers.clone()) => {}
        _ = tokio::signal::ctrl_c() => log.line("STOP", "ctrl+c"),
    }

    // Never leave test binds behind.
    if let Ok(Some(lockfile)) = Lockfile::read(install.root()) {
        let client = LcuClient::new(&lockfile)?;
        restore_markers(&client, &log, &markers).await;
        if apply_test {
            if let Ok(settings) = client.get::<Value>("/lol-game-settings/v1/input-settings").await {
                for (_, event, _) in MARKERS {
                    let bind = settings.pointer(&format!("/GameEvents/{event}")).cloned().unwrap_or_default();
                    log.line("APPLYTEST", &format!("at exit {event} = {bind}"));
                }
            }
        }
    }
    let counts = log.uri_counts.lock().unwrap().clone();
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    for (uri, count) in counts {
        log.file_only("URICOUNT", &format!("{count:>6}  {uri}"));
    }
    Ok(())
}

/// Logs every write to the settings files and, for the JSON, exactly what changed.
async fn watch_files(install: LeagueInstall, log: Log) {
    let config = install.root().join("Config");
    let files: Vec<PathBuf> =
        ["PersistedSettings.json", "game.cfg", "input.ini"].iter().map(|name| config.join(name)).collect();
    let modified = |path: &PathBuf| std::fs::metadata(path).and_then(|meta| meta.modified()).ok();
    let read_settings = || -> Option<SettingsMap> {
        let text = std::fs::read_to_string(install.persisted_settings()).ok()?;
        Some(SettingsMap::from(&serde_json::from_str::<PersistedSettings>(&text).ok()?))
    };

    let mut seen: Vec<_> = files.iter().map(modified).collect();
    let mut settings = read_settings();
    loop {
        tokio::time::sleep(Duration::from_millis(250)).await;
        for (index, path) in files.iter().enumerate() {
            let now = modified(path);
            if now == seen[index] {
                continue;
            }
            seen[index] = now;
            let size = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
            log.line("FILE", &format!("{} written ({size} bytes)", path.file_name().unwrap().to_string_lossy()));

            if index == 0 {
                // The file may be caught half-written; give the writer a moment.
                tokio::time::sleep(Duration::from_millis(150)).await;
                let Some(current) = read_settings() else {
                    log.line("FILEDIFF", "could not parse PersistedSettings.json");
                    continue;
                };
                if let Some(previous) = &settings {
                    let changes = diff(previous, &current);
                    log.line("FILEDIFF", &format!("{} keys changed", changes.len()));
                    for change in &changes {
                        let detail = format!(
                            "{}/{}/{}: {:?} -> {:?}",
                            change.file, change.section, change.key, change.from, change.to
                        );
                        // An account switch changes nearly everything; keep that off the console.
                        log.write("FILEDIFF", &detail, changes.len() <= 12);
                    }
                }
                settings = Some(current);
            }
        }
    }
}

/// Logs when the game itself starts and exits. The game reads settings at launch, so
/// its start is the real deadline for applying them. Uses a process-list snapshot only;
/// the game process is never opened.
async fn watch_game_process(log: Log) {
    let mut running = game_is_running();
    if running {
        log.line("GAMEPROC", "already running");
    }
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let now = game_is_running();
        if now != running {
            running = now;
            log.line("GAMEPROC", if now { "started" } else { "exited" });
        }
    }
}

/// Overridable so the watcher can be tested without starting a game.
fn game_process() -> String {
    std::env::var("MIMIC_GAME_PROCESS").unwrap_or_else(|_| GAME_PROCESS.to_owned())
}

fn game_is_running() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
    };

    // SAFETY: the snapshot handle is checked, used only with the ToolHelp functions and
    // closed; `entry` is zeroed with `dwSize` set as the API requires.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        let mut more = Process32FirstW(snapshot, &mut entry);
        while more != 0 {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(&game_process()) {
                found = true;
                break;
            }
            more = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        found
    }
}

/// Connects whenever a client is up and reconnects after it goes away.
async fn follow_client(install: LeagueInstall, log: Log, apply_test: bool, markers: Arc<Mutex<Markers>>) {
    loop {
        let (lockfile, client) = loop {
            if let Ok(Some(lockfile)) = Lockfile::read(install.root()) {
                if let Ok(client) = LcuClient::new(&lockfile) {
                    if client.is_alive().await {
                        break (lockfile, client);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        };
        log.line("CLIENT", &format!("up pid={} port={}", lockfile.pid, lockfile.port));

        // Wait out the login: right after start the client answers but knows no account yet.
        let mut puuid = String::new();
        for _ in 0..60 {
            if let Ok(summoner) = client.current_summoner().await {
                puuid = summoner.puuid;
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        log.line("ACCOUNT", &format!("puuid={}...", prefix(&puuid)));

        // `--only-puuid <prefix>` keeps the apply test away from every other account.
        let args: Vec<String> = std::env::args().collect();
        let only = args.iter().position(|arg| arg == "--only-puuid").and_then(|at| args.get(at + 1));
        let apply_test = apply_test && !puuid.is_empty() && only.is_none_or(|only| puuid.starts_with(only.as_str()));
        log.line("APPLYTEST", if apply_test { "enabled for this account" } else { "disabled for this account" });
        if let Ok(phase) = client.gameflow_phase().await {
            log.line("PHASE", &phase);
        }
        if let Ok(reset) = client.did_reset().await {
            log.line("DIDRESET", &reset.to_string());
        }

        match lcu::subscribe(&lockfile).await {
            Ok(events) => {
                let mut events = std::pin::pin!(events);
                let mut state = ClientState::default();
                while let Some(event) = events.next().await {
                    match event {
                        Ok(event) => {
                            let moment = log_event(&log, &event, &mut state);
                            if let (true, Some(moment)) = (apply_test, moment) {
                                handle_moment(&client, &log, &markers, moment).await;
                            }
                        }
                        Err(err) => {
                            log.line("WS", &format!("error: {err}"));
                            break;
                        }
                    }
                }
            }
            Err(err) => log.line("WS", &format!("connect failed: {err}")),
        }
        log.line("CLIENT", "down");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[derive(Default)]
struct ClientState {
    champ_select: String,
    champ_select_dumped: bool,
    last_session: Option<Value>,
    gameflow: String,
    champion_locked: bool,
    /// Last payload per settings URI, to log what an update changed.
    settings: BTreeMap<String, Value>,
}

/// A point in the game flow the apply test reacts to.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Moment {
    ChampionLocked,
    GameStart,
    InProgress,
    GameOver,
}

fn log_event(log: &Log, event: &LcuEvent, state: &mut ClientState) -> Option<Moment> {
    let uri = event.uri.as_str();
    *log.uri_counts.lock().unwrap().entry(uri.to_owned()).or_default() += 1;
    let mut moment = None;

    if uri == "/lol-gameflow/v1/gameflow-phase" {
        let phase = event.data.as_str().unwrap_or("?");
        log.line("PHASE", phase);
        moment = match phase {
            "GameStart" => Some(Moment::GameStart),
            "InProgress" => Some(Moment::InProgress),
            // Not the earlier post-game phases: the game may still be writing settings on exit.
            "EndOfGame" | "Lobby" | "None" => Some(Moment::GameOver),
            _ => None,
        };
        if phase == "ChampSelect" {
            state.champion_locked = false;
        }
    } else if uri == "/lol-gameflow/v1/session" {
        let summary = gameflow_summary(&event.data);
        if summary != state.gameflow {
            log.line("GAMEFLOW", &summary);
            state.gameflow = summary;
        }
    } else if uri == "/lol-champ-select/v1/session" {
        if event.event_type == "Delete" {
            log.line("CHAMPSEL", "session deleted");
            if let Some(last) = state.last_session.take() {
                log.file_only("CHAMPSEL", &format!("final session: {last}"));
            }
            state.champ_select.clear();
            state.champ_select_dumped = false;
        } else {
            if !state.champ_select_dumped {
                state.champ_select_dumped = true;
                log.file_only("CHAMPSEL", &format!("first session: {}", event.data));
            }
            let (summary, champion_id, raw) = champ_select_summary(&event.data);
            // The session updates constantly; only changes to what we care about matter.
            if summary != state.champ_select {
                log.line("CHAMPSEL", &summary);
                log.file_only("CHAMPSEL", &format!("me: {raw}"));
                state.champ_select = summary;
            }
            // Draft and blind pick assign the champion at lock-in, ARAM from the start.
            if champion_id != 0 && !state.champion_locked {
                state.champion_locked = true;
                moment = Some(Moment::ChampionLocked);
            }
            state.last_session = Some(event.data.clone());
        }
    } else if uri.starts_with("/lol-champ-select/v1/") && !uri.contains("/summoners/") {
        let data = event.data.to_string();
        if data.len() <= 300 {
            log.line("CHAMPSEL", &format!("{} {uri} {data}", event.event_type));
        }
    } else if uri == "/lol-summoner/v1/current-summoner" {
        let puuid = event.data.get("puuid").and_then(Value::as_str).unwrap_or("");
        log.line("ACCOUNT", &format!("{} puuid={}...", event.event_type, prefix(puuid)));
    } else if uri.starts_with("/lol-game-settings/") {
        log.line("SETTINGS", &format!("{} {uri}{}", event.event_type, settings_change(&event.data, state, uri)));
    } else if uri.starts_with("/lol-settings/") {
        log.line("PREFS", &format!("{} {uri}", event.event_type));
    } else if uri.starts_with("/lol-end-of-game/") {
        log.line("ENDGAME", &format!("{} {uri}", event.event_type));
    }
    moment
}

/// What changed in a settings payload since the last event for the same URI.
fn settings_change(data: &Value, state: &mut ClientState, uri: &str) -> String {
    let previous = state.settings.insert(uri.to_owned(), data.clone());
    let (Some(previous), Some(sections)) = (previous, data.as_object()) else {
        return match data {
            Value::Bool(_) | Value::Number(_) | Value::String(_) => format!(" = {data}"),
            _ => String::new(),
        };
    };
    let mut changes = Vec::new();
    for (section, settings) in sections {
        for (key, value) in settings.as_object().into_iter().flatten() {
            let old = previous.get(section).and_then(|section| section.get(key));
            if old != Some(value) {
                changes.push(format!("{section}/{key}: {} -> {value}", old.unwrap_or(&Value::Null)));
            }
        }
    }
    if changes.is_empty() {
        " (no change)".to_owned()
    } else {
        format!(" [{}]", changes.join(", "))
    }
}

fn gameflow_summary(session: &Value) -> String {
    let text = |pointer: &str| session.pointer(pointer).map(Value::to_string).unwrap_or_else(|| "?".into());
    format!(
        "phase={} queue={} mode={} map={} custom={} gameId={} client.running={} client.visible={}",
        text("/phase"),
        text("/gameData/queue/id"),
        text("/gameData/queue/gameMode"),
        text("/map/id"),
        text("/gameData/isCustomGame"),
        text("/gameData/gameId"),
        text("/gameClient/running"),
        text("/gameClient/visible"),
    )
}

/// The local player's champion, hover intent, lock state and the timer phase.
fn champ_select_summary(session: &Value) -> (String, i64, String) {
    let cell = session.get("localPlayerCellId").and_then(Value::as_i64);
    let me = session
        .get("myTeam")
        .and_then(Value::as_array)
        .and_then(|team| team.iter().find(|player| player.get("cellId").and_then(Value::as_i64) == cell));
    let field = |name: &str| me.and_then(|me| me.get(name)).and_then(Value::as_i64).unwrap_or(0);
    let timer = session.pointer("/timer/phase").and_then(Value::as_str).unwrap_or("?");
    let time_left = session.pointer("/timer/adjustedTimeLeftInPhase").and_then(Value::as_i64).unwrap_or(-1);
    let bench = session.get("benchChampions").and_then(Value::as_array).map_or(0, Vec::len);
    // `actions` is a list of rounds, each a list of actions.
    let locked = session
        .get("actions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .flatten()
        .any(|action| {
            action.get("actorCellId").and_then(Value::as_i64) == cell
                && action.get("type").and_then(Value::as_str) == Some("pick")
                && action.get("completed").and_then(Value::as_bool) == Some(true)
        });
    let champion_id = field("championId");
    let summary = format!(
        "championId={champion_id} intent={} locked={locked} timer={timer} bench={bench}",
        field("championPickIntent")
    );
    // `time_left` changes on every event, so it stays out of the deduplicated summary.
    let raw = format!("timeLeftMs={time_left} {}", me.map(Value::to_string).unwrap_or_default());
    (summary, champion_id, raw)
}

fn prefix(puuid: &str) -> &str {
    &puuid[..8.min(puuid.len())]
}

// Apply test

/// Original values of the marker binds that are currently overridden.
#[derive(Default)]
struct Markers {
    originals: BTreeMap<&'static str, String>,
}

async fn handle_moment(client: &LcuClient, log: &Log, markers: &Arc<Mutex<Markers>>, moment: Moment) {
    let index = match moment {
        Moment::ChampionLocked => 0,
        Moment::GameStart => 1,
        Moment::InProgress => 2,
        Moment::GameOver => return restore_markers(client, log, markers).await,
    };
    let (label, event, key) = MARKERS[index];
    if markers.lock().unwrap().originals.contains_key(event) {
        return;
    }
    let original = match client.get::<Value>("/lol-game-settings/v1/input-settings").await {
        Ok(settings) => settings.pointer(&format!("/GameEvents/{event}")).and_then(Value::as_str).unwrap_or("").to_owned(),
        Err(err) => return log.line("APPLYTEST", &format!("{label}: could not read current bind: {err}")),
    };
    match patch_bind(client, event, key).await {
        Ok(()) => {
            markers.lock().unwrap().originals.insert(event, original.clone());
            log.line("APPLYTEST", &format!("{label}: {event} {original:?} -> {key:?}, saved"));
        }
        Err(err) => log.line("APPLYTEST", &format!("{label}: failed: {err}")),
    }
}

async fn restore_markers(client: &LcuClient, log: &Log, markers: &Arc<Mutex<Markers>>) {
    let originals = std::mem::take(&mut markers.lock().unwrap().originals);
    for (event, original) in originals {
        match patch_bind(client, event, &original).await {
            Ok(()) => log.line("APPLYTEST", &format!("restored {event} to {original:?}")),
            Err(err) => {
                log.line("APPLYTEST", &format!("COULD NOT RESTORE {event} to {original:?}: {err}"));
                markers.lock().unwrap().originals.insert(event, original);
            }
        }
    }
}

async fn patch_bind(client: &LcuClient, event: &str, key: &str) -> lcu::Result<()> {
    let body = json!({ "GameEvents": { event: key } });
    let _: Value = client.patch("/lol-game-settings/v1/input-settings", &body).await?;
    client.save_settings().await?;
    Ok(())
}

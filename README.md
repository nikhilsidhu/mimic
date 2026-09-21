# mimic

A Windows tray app that saves your League of Legends settings as profiles and applies them for you:

- **Across accounts.** Keybinds, camera, HUD and sound settings follow you when you log into another account. An account can apply its profile by itself at every login.
- **Per champion.** A champion can have its own overrides. They go on when you pick the champion and come off after the game.

You keep changing settings in the game itself. After a game mimic shows what changed and asks where it should go: into your profile, to that champion only, kept on this account, or reverted.

> **Status: early, but usable.** The core works and is in daily use by its author. Per-champion overrides have had little testing in real games. Expect rough edges.

## What it does

- Saves the logged-in account's settings as a profile, applies a profile to any account, and keeps a snapshot before every change so it can be undone.
- Shows a change log of exactly which settings each change altered, with keybinds drawn as keys.
- Notices when Riot resets your settings after a patch and offers to put yours back.
- Exports and imports profiles as single files.
- Lives in the tray. One panel for everyday use, a manager window for the rest.

## How it works

League stores your in-game settings per account on Riot's servers and downloads them at login, so copying config files between accounts does not stick. mimic writes settings through the League client's local API (the LCU) while you are logged in, and the client syncs them to the account.

mimic only talks to the League client and reads config files. It never reads or modifies the game process and does not automate anything in champ select or in game.

Everything is stored locally as plain JSON under `%APPDATA%\mimic`. There is no account, no server and no telemetry. Champion names and icons are fetched from your own League client and cached; none of Riot's assets ship with mimic.

Known limits: the client refuses a few keybinds when they conflict with keys it reserves, such as push-to-talk; mimic reports those instead of pretending they applied. Settings changed while a game is running take effect in the next game, as the game reads them once while loading.

## Install

Installers are attached to [releases](https://github.com/nikhilsidhu/mimic/releases). The installer is per user and needs no administrator rights. It is not code-signed yet, so Windows SmartScreen will warn about it.

## Development

Requirements: Windows, [Rust](https://rustup.rs) (MSVC toolchain), Visual Studio Build Tools 2022 with "Desktop development with C++", and Node.js LTS. Use a native Windows shell, not WSL.

```powershell
npm install
npm run tauri dev                 # run the app
npm run check                     # type-check the frontend
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build               # build the installer
```

`cargo run --example lcu_log` in `src-tauri` logs the client events mimic depends on, for measuring their timing across real games. `scripts/capture-windows.ps1` screenshots mimic's windows.

## Legal

mimic isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games, and all associated properties are trademarks or registered trademarks of Riot Games, Inc.

Licensed under the [MIT License](LICENSE). The bundled font, Ioskeley Mono, is under the [SIL Open Font License](src/lib/fonts/ioskeley-mono/OFL.txt).

# mimic

A Windows tray app that saves your League of Legends settings as profiles and applies them for you:

- **Across accounts.** Keybinds, camera, HUD and sound settings follow you when you log into another account.
- **Per champion.** A champion can have its own overrides, applied automatically during champ select and reverted after the game.

You keep changing settings in the game itself. mimic notices what changed and asks whether to save it.

> **Status: early development.** Nothing here is usable yet.

## How it works

League stores your in-game settings per account on Riot's servers and downloads them at login, so copying config files between accounts does not stick. mimic writes settings through the League client's local API (the LCU) while you are logged in, and the client syncs them to the account.

mimic only talks to the League client and reads config files. It never reads or modifies the game process and does not automate anything in champ select or in game.

Everything is stored locally as plain JSON. There is no account, no server and no telemetry.

## Development

Requirements: Windows, [Rust](https://rustup.rs) (MSVC toolchain), Visual Studio Build Tools 2022 with "Desktop development with C++", and Node.js LTS. Use a native Windows shell, not WSL.

```powershell
npm install
npm run tauri dev                # run the app
cargo test --manifest-path src-tauri/Cargo.toml
```

## Legal

mimic isn't endorsed by Riot Games and doesn't reflect the views or opinions of Riot Games or anyone officially involved in producing or managing Riot Games properties. Riot Games, and all associated properties are trademarks or registered trademarks of Riot Games, Inc.

Licensed under the [MIT License](LICENSE).

# Claude Sync

A robust, private background synchronization tool for your Claude Desktop (`claude code`) settings and commands across multiple machines. Built securely with Tauri, React, and Rust.

## Features

- **Manual Push / Pull Model:** Control your data synchronizations precisely without risking conflicts from auto-file-watchers.
- **Secure Persistence:** Uses the OS System Keyring (Windows Credential Manager) to encrypt and safely store your Github Personal Access Tokens instead of retaining them raw.
- **Zero-Friction Setup:** Can automatically establish a private `claude-settings` repository directly via GitHub API.
- **Auto-Boot Background Process:** Operates frictionlessly out of your system tray on boot if enabled.
- **Granular Sync Settings:** Granularly toggles optional syncing for `.credentials.json` with the rest of your agents and custom prompts (`CLAUDE.md`, etc.).

## Setup

1. Generate a GitHub Personal Access Token (`repo` scoped).
2. Start Claude Sync.
3. If you do not have a Settings repository, simply click **Create Repo** inside the setup screen. It will provision `claude-settings` automatically for you on Github.
4. Click `Save & Start`.
5. Operate strictly via your System Tray to push state or pull state to synchronize variables and settings across all your working machines!

## Monitored Files

* `settings.json`
* `.claude.json`
* `CLAUDE.md`
* `/commands/`
* `/agents/`
* `/skills/`
* `/plugins/`
* *(Optional)* `.credentials.json`

## Development

Requires Node (`pnpm`) and Rust to build.

```bash
# Install dependencies
pnpm install

# Run the dev instance
pnpm tauri dev

# Build the system installer (.msi or .exe bundle)
pnpm tauri build
```

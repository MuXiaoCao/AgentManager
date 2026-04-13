# AgentManager

A macOS desktop dashboard for Claude Code sessions. A left-docked side panel shows real-time session status with one-click iTerm jumping, window arrangement, and session history restore.

[中文](./README.md)

![macOS](https://img.shields.io/badge/macOS-12.0%2B-blue) ![License](https://img.shields.io/badge/license-MIT-green)

**Tauri 2 + Rust + React 18 + TypeScript**

## Features

- **Live session cards** — Five hooks (SessionStart/Stop/Notification/SessionEnd/UserPromptSubmit), sessions appear the moment Claude starts
- **Click to jump** — Focus the exact iTerm window/tab/split pane
- **Arrange windows** — One click to tile all iTerm windows into a grid (stays on current desktop, no Space switching)
- **Rename** — Rename cards and Claude history entries for easier lookup
- **Claude History** — Scan all sessions from `~/.claude/`, click to resume (`claude --resume`)
- **Session persistence** — Survives app restarts, up to 200 entries
- **i18n** — English / 中文 toggle with 🌐 button

## Install

### Option A: Download DMG (recommended)

1. Go to [**Releases**](https://github.com/MuXiaoCao/AgentManager/releases/latest) and download the latest DMG
2. Open DMG, drag `AgentManager.app` to `/Applications`
3. First launch: right-click → Open → click "Open" in the Gatekeeper dialog
4. Click the blue **"Install hook"** banner at the top to set up Claude Code hooks
5. **Restart any running Claude sessions** for hooks to take effect

### Option B: Build from source

```bash
git clone https://github.com/MuXiaoCao/AgentManager.git
cd AgentManager
npm install
npm run tauri:build
cp -R src-tauri/target/release/bundle/macos/AgentManager.app /Applications/
```

### Requirements

- macOS 12.0+ (Apple Silicon or Intel)
- [iTerm2](https://iterm2.com/)
- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)
- `jq` (`brew install jq`)

## Documentation

| Document | Content |
|----------|---------|
| [Setup & Development Guide](docs/guides/setup.md) | Installation, development, hook config, permissions |
| [Architecture](docs/architecture/overview.md) | Data flows, design decisions, tech stack |
| [AI Work Protocol](AGENTS.md) | Agent harness, constraints, plan workflow |
| [Rust Module SPEC](src-tauri/SPEC.md) | Backend file responsibilities, key decisions |
| [React Module SPEC](src/SPEC.md) | Component tree, state management |
| [Plan Guidelines](docs/plans/README.md) | Implementation plan format and workflow |

## License

MIT

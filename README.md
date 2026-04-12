# AgentManager

A macOS desktop dashboard for Claude Code sessions. Built with Tauri 2 + Rust + React.

![macOS](https://img.shields.io/badge/macOS-12.0%2B-blue) ![License](https://img.shields.io/badge/license-MIT-green)

## Install (macOS)

### Option A: Download DMG (recommended)

1. Go to [**Releases**](https://github.com/MuXiaoCao/AgentManager/releases/latest)
2. Download `AgentManager_x.x.x_aarch64.dmg` (Apple Silicon) or `AgentManager_x.x.x_x64.dmg` (Intel)
3. Open the DMG, drag `AgentManager.app` into `/Applications`
4. First launch: right-click → Open → click "Open" in the dialog (Gatekeeper unsigned app)
5. AgentManager will auto-dock to the left edge of your screen

### Option B: Build from source

```bash
# Prerequisites: Rust 1.77+, Node 18+, Xcode CLI tools, jq
git clone https://github.com/MuXiaoCao/AgentManager.git
cd AgentManager
npm install
npm run tauri:build
# Output: src-tauri/target/release/bundle/macos/AgentManager.app
cp -R src-tauri/target/release/bundle/macos/AgentManager.app /Applications/
```

### Setup Claude Code Hook

On first launch, if the hook isn't installed yet, a blue banner appears at the top — click **"Install hook"** and you're done. Or run it manually:

```bash
open -a AgentManager
# Then click the blue "Install hook" / "一键安装" banner
```

This writes `~/.claude-dashboard/hook.sh` and adds `SessionStart` + `Stop` + `SessionEnd` hooks to `~/.claude/settings.json`. Existing hooks are preserved. **Restart any running Claude sessions** for the hooks to take effect.

## Features

### Dashboard (面板)

- **Left-docked panel** — auto-pins to the left edge at full screen height on launch.
- **Live session cards** — Claude sessions appear the moment they start (via `SessionStart` hook), not just when they end.
- **Click a card** → jumps to the corresponding iTerm window/tab/session and brings it to the foreground.
- **Right-click a card** →
  - **Rename** — inline editable alias, persisted across restarts.
  - **Jump to iTerm** — focuses the exact split pane.
  - **Arrange all iTerm windows** — tiles every tracked session's window into a grid on the area right of the dashboard.
  - **Dismiss** / **Delete from history**.
- **Session history persists** — survived cards are saved to `~/Library/Application Support/agent-manager/sessions.json` and restored on restart. Ended sessions can be reopened with `claude --resume`.
- **Worktree dedup** — when you switch worktrees in the same iTerm pane, the old card is automatically replaced.

### Claude History (Claude 历史)

A second tab scans `~/.claude/` to show **all** Claude Code sessions ever run on this machine:

- **Session metadata** from `~/.claude/sessions/*.json`
- **Conversation files** from `~/.claude/projects/<project>/<session>.jsonl`
- First user prompt extracted as a **summary preview** (reads ≤64KB per file)
- **Search** by project path, prompt text, or session ID
- **Click any entry** → opens a new iTerm window and runs `claude --resume <session_id>` in the original cwd

### Other

- **i18n** — English / 中文 toggle (🌐 button in header), persisted in localStorage.
- **Automatic hook installer** — one-click, idempotent, preserves existing hooks.

## Architecture

```
┌──────────────────────────────────────────────────┐
│               AgentManager                       │
│          (Tauri 2 desktop app)                   │
├──────────────────────────────────────────────────┤
│  React frontend  ◄─IPC─►  Rust backend           │
│  - Tab bar (Dashboard / Claude History)          │
│  - Session cards          - DashMap state        │
│  - Context menu           - axum HTTP :19280     │
│  - Inline rename          - AppleScript bridge   │
│  - i18n (en/zh)           - Claude history scan  │
└───────────────────┬──────────────────────────────┘
                    │ HTTP POST /api/notify
                    ▼
         ~/.claude-dashboard/hook.sh
                    ▲
                    │ stdin JSON (hook_event_name, session_id, cwd, ...)
              Claude Code hooks
         (SessionStart / Stop / SessionEnd)
```

## Tauri commands

| Command | Description |
|---|---|
| `get_sessions` | Returns all tracked sessions, newest first |
| `dismiss_session` | Remove from active dashboard |
| `delete_session` | Permanently remove from history + disk |
| `rename_session` | Set/clear a display alias (persisted) |
| `reopen_session` | Open new iTerm + `claude --resume` |
| `jump_to_iterm` | Focus the session's iTerm window/tab/pane |
| `arrange_iterm_windows` | Tile tracked windows into a grid |
| `list_claude_sessions` | Scan `~/.claude/` for all historical sessions |
| `check_hook_config` | Report installed hook events |
| `install_claude_hook` | Idempotently write hook.sh + settings.json |

## HTTP API

```
POST http://127.0.0.1:19280/api/notify
Content-Type: application/json

{
  "session_id": "abc-123",
  "cwd": "/path/to/project",
  "iterm_session_id": "w0t0p0:UUID",
  "event_type": "sessionstart",
  "agent": "claude"
}

→ 200 ok
```

## Data files

| Path | Purpose |
|---|---|
| `~/Library/Application Support/agent-manager/sessions.json` | Persisted session state (max 200) |
| `~/Library/Application Support/agent-manager/aliases.json` | Card rename aliases |
| `~/.claude-dashboard/hook.sh` | Shared hook script |
| `~/.claude/settings.json` | Claude Code hooks config (modified by installer) |

## Development

```bash
npm install
npm run tauri:dev      # Vite + Tauri with HMR
npm run tauri:build    # release .app + .dmg + auto-patch Info.plist
```

## License

MIT

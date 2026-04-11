# AgentManager

A macOS desktop dashboard for Claude Code sessions. Built with Tauri 2 + Rust +
React. Session data flows in through a local HTTP hook (`POST /api/notify`),
gets rendered as cards in a left-docked side panel, and each card can jump
back to its original iTerm session with a double click.

## Features

- **Left-docked panel** — on launch, the main window pins to the left edge of
  the primary monitor and stretches to full screen height, so it always stays
  visible next to your terminals.
- **Right-click on any card** to:
  - **Rename** — assign a friendly alias that persists across launches
    (`~/.config/agent-manager/aliases.json`).
  - **Jump to iTerm** — `activate` iTerm and focus the exact tab/session the
    card represents (uses the session's `ITERM_SESSION_ID`).
  - **Arrange all iTerm windows** — tiles every tracked session's iTerm window
    into a grid on the remaining screen area (the left dock strip is reserved
    for the main window).
  - **Dismiss** — remove a stale card.
- **Automatic hook setup** — a one-click installer writes the shared
  `~/.claude-dashboard/hook.sh` script and registers it under **three** Claude
  events in `~/.claude/settings.json`: `SessionStart`, `Stop`, `SessionEnd`.
  Existing unrelated hooks are preserved and additions are idempotent.
- **Live updates** via a Tauri event (`session-updated`) — no polling.

## Why three hook events?

The original inspiration for this rewrite (a closed-source app called
AgentPulse) only installed `Stop` and `SessionEnd` hooks, which meant running
Claude sessions never appeared in the dashboard until they exited. AgentManager
adds `SessionStart` so the card shows up the moment you launch Claude.

## Architecture

```
┌──────────────────────────────────────────────────┐
│               AgentManager                       │
│          (Tauri 2 desktop app)                   │
├──────────────────────────────────────────────────┤
│  React frontend  ◄─IPC─►  Rust backend           │
│  - Session cards          - DashMap state        │
│  - Context menu           - axum HTTP server     │
│  - Toasts                 - AppleScript bridge   │
└───────────────────┬──────────────────────────────┘
                    │ HTTP POST /api/notify
                    ▼
         ~/.claude-dashboard/hook.sh
                    ▲
                    │ stdin JSON (hook_event_name, session_id, cwd, ...)
                    │
              Claude Code hooks
         (SessionStart / Stop / SessionEnd)
```

## Tauri commands

| Command | Description |
|---|---|
| `get_sessions` | Returns all tracked sessions, newest first. |
| `dismiss_session(session_id)` | Removes a session from the dashboard. |
| `rename_session(session_id, alias)` | Set/clear a user alias; persisted. |
| `jump_to_iterm(session_id)` | Activates iTerm and focuses the matching tab. |
| `arrange_iterm_windows()` | Tiles every tracked session's iTerm window. |
| `check_hook_config()` | Reports which events are currently installed. |
| `install_claude_hook()` | Idempotently writes hook.sh + settings.json. |

## HTTP API

```
POST /api/notify
Content-Type: application/json

{
  "session_id": "abc123",
  "cwd": "/path/to/project",
  "iterm_session_id": "A1B2C3...",
  "event_type": "sessionstart",
  "agent": "claude"
}
```

Responds with `200 ok` on success, `422` on malformed input. A
`session-updated` event is emitted to the frontend each time the state mutates.

## Development

Requirements: Rust 1.77+, Node 18+, Xcode command line tools, `jq` (for the
hook script at runtime), `osascript` (ships with macOS).

```bash
npm install
npm run tauri:dev      # runs Vite + Tauri with HMR
npm run tauri:build    # produces release .app and .dmg
```

A debug build lives at:
```
src-tauri/target/debug/bundle/macos/AgentManager.app
```

## License

MIT

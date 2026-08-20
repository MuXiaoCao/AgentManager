# Agent Notifications Design

**Goal:** AgentManager should actively remind the user when an agent needs attention or finishes a long-running turn.

**Tech Stack / Impact:** Rust session state and notification commands, Tauri window focus APIs, React toolbar/settings UI, existing Claude/Codex hook events.

---

## Scope

First version only handles Claude/Codex hook events already managed by AgentManager:

- `Stop`: the current agent turn has finished and the session is idle.
- `Notification`: Claude needs user attention.
- `PermissionRequest`: Codex needs user attention; internally treated like `Notification`.

It does not monitor arbitrary terminal commands such as `npm run build` or `cargo test`. Sessions that were already running before hooks were installed still cannot provide live completion reminders.

---

## Reminder Rules

### Long Turn Finished

On `UserPromptSubmit`, record `turn_started_at` for the session. On `Stop`, calculate elapsed time and remind only when all conditions match:

- `turn_started_at` exists.
- elapsed time is greater than or equal to the configured threshold.
- AgentManager is not the foreground app.
- the same session has not emitted a recent duplicate finish reminder.

Default threshold: `30s`. The threshold should be configurable.

### Needs Attention

On Claude `Notification` or Codex `PermissionRequest`, remind immediately:

- If AgentManager is foreground, show an in-app toast or lightweight overlay.
- If AgentManager is background, send a macOS system notification.
- Apply short debounce per session, for example `5s`, to avoid repeated permission prompts flooding notifications.

---

## Notification Behavior

Notification title examples:

- `Codex 已完成`
- `Claude 需要处理`

Body should include the project directory name and the current preview text when available.

Click behavior:

- Prefer jumping to the tracked iTerm pane using existing `jump_to_iterm`.
- If `iterm_session_id` is missing or `unknown`, focus AgentManager and select the matching card.

---

## Configuration

Add a small settings surface instead of a full settings page:

- `完成后提醒`: default on.
- `需要介入提醒`: default on.
- `完成提醒阈值`: default `30s`.

Persist settings under AgentManager config, alongside current order/alias/history state.

---

## Implementation Notes

- Extend `AppState` with per-session reminder runtime state: `turn_started_at`, `last_finish_reminded_at`, `last_attention_reminded_at`.
- Keep persisted user settings separate from runtime state.
- Emit a frontend event when the backend decides an in-app reminder should be shown.
- Use Tauri notification APIs for background macOS notifications.
- Preserve existing card status behavior: `Stop` remains idle, not ended.

---

## Verification Checklist

- [ ] `cargo test --lib`
- [ ] `npx tsc --noEmit`
- [ ] `npm run build`
- [ ] `npm run tauri:build`
- [ ] Manual: foreground `Notification` shows in-app reminder.
- [ ] Manual: background `PermissionRequest` sends macOS notification.
- [ ] Manual: short `Stop` under threshold does not notify.
- [ ] Manual: long `Stop` over threshold notifies only when AgentManager is background.
- [ ] Manual: clicking notification jumps to the correct iTerm pane when available.

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::process::Command;
use tauri::{command, Emitter, Manager, State};

use crate::claude_history::{self, ClaudeHistoryEntry};
use crate::hook_install::{self, CombinedHookInstallReport, CombinedHookStatus, HookInstallReport};
use crate::http_server::{HttpServerHealth, HttpServerStatus};
use crate::iterm::{self, ArrangeReport, TileRegion};
use crate::state::{AppState, SessionEntry};

#[command]
pub fn get_sessions(state: State<'_, AppState>) -> Vec<SessionEntry> {
    let mut sessions = state.list_sessions();
    let mut seen: HashSet<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
    append_scanned_history(&state, &mut sessions, &mut seen, "claude");
    append_scanned_history(&state, &mut sessions, &mut seen, "codex");
    sessions
}

#[command]
pub fn dismiss_session(state: State<'_, AppState>, session_id: String) -> bool {
    state.dismiss(&session_id)
}

/// Clear all ended sessions from history.
#[command]
pub fn clear_history(state: State<'_, AppState>) {
    state.clear_history();
    let ids = scanned_history_ids();
    state.hide_history_sessions(&ids);
}

/// Permanently remove a session from history (disk + memory).
#[command]
pub fn delete_session(state: State<'_, AppState>, session_id: String) -> bool {
    state.delete_session(&session_id)
}

/// Open a new iTerm window and run `claude --resume <session_id>` inside
/// the given cwd. If `cwd` is not provided, looks it up from AgentManager's
/// tracked state. This allows both Dashboard history cards and Claude history
/// tab entries to use the same command.
#[command]
pub fn reopen_session(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    cwd: Option<String>,
    agent: Option<String>,
) -> Result<(), String> {
    let stored = state.sessions.get(&session_id).map(|r| r.value().clone());
    let effective_agent = agent
        .filter(|s| !s.is_empty())
        .or_else(|| stored.as_ref().map(|entry| entry.agent.clone()))
        .unwrap_or_else(|| "claude".to_string());
    let effective_cwd = cwd
        .filter(|s| !s.is_empty())
        .or_else(|| stored.as_ref().map(|entry| entry.cwd.clone()))
        .unwrap_or_default();
    if effective_cwd.is_empty() {
        return Err(format!("no cwd available for session {session_id}"));
    }
    let iterm_session_id = iterm::reopen_session(&effective_cwd, &session_id, &effective_agent)
        .map_err(|e| e.to_string())?;
    let entry = state.upsert_from_notify(crate::state::NotifyPayload {
        session_id,
        cwd: effective_cwd,
        iterm_session_id,
        event_type: "sessionstart".to_string(),
        agent: effective_agent,
    });
    let _ = app.emit("session-updated", &entry);
    Ok(())
}

/// Persist user's custom card order (from drag-and-drop reordering).
#[command]
pub fn reorder_sessions(state: State<'_, AppState>, order: Vec<String>) {
    state.reorder_sessions(&order);
}

/// Clear the notification badge on a card (mark all as handled).
#[command]
pub fn clear_notifications(state: State<'_, AppState>, session_id: String) {
    if let Some(mut entry) = state.sessions.get_mut(&session_id) {
        entry.notification_count = 0;
    }
    let _ = state.save_sessions_pub();
}

/// Persist a display alias for `session_id`. This is purely cosmetic —
/// it only affects the card title inside AgentManager, not the iTerm tab.
#[command]
pub fn rename_session(state: State<'_, AppState>, session_id: String, alias: Option<String>) {
    state.set_alias(&session_id, alias);
}

#[command]
pub fn jump_to_iterm(
    state: State<'_, AppState>,
    session_id: String,
    pulse: Option<bool>,
) -> Result<(), String> {
    let Some(entry) = state.sessions.get(&session_id).map(|r| r.value().clone()) else {
        return Err(format!("session {session_id} not found"));
    };
    iterm::jump_to(&entry.iterm_session_id, pulse.unwrap_or(false)).map_err(|e| e.to_string())
}

/// Arrange iTerm windows into a grid. If `iterm_session_ids` is provided
/// (from the frontend's card order), windows are reordered to match before
/// tiling. Otherwise all iTerm windows are tiled in their current z-order.
#[command]
pub fn arrange_iterm_windows(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ArrangeReport, String> {
    let region = compute_region(&app).map_err(|e| e.to_string())?;

    // Get active sessions in card order → their iterm_session_ids.
    let sessions = state.list_sessions();
    let ordered_iterm_ids: Vec<String> = sessions
        .iter()
        .filter(|s| s.last_event != "sessionend")
        .filter(|s| !s.iterm_session_id.is_empty() && s.iterm_session_id != "unknown")
        .map(|s| s.iterm_session_id.clone())
        .collect();

    iterm::arrange_windows(region, &ordered_iterm_ids).map_err(|e| e.to_string())
}

fn compute_region(app: &tauri::AppHandle) -> anyhow::Result<TileRegion> {
    use tauri::PhysicalPosition;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow::anyhow!("main window missing"))?;
    // Use the monitor AgentManager is CURRENTLY on, not primary_monitor().
    // The user may have dragged AgentManager to a secondary display.
    let monitor = window
        .current_monitor()?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| anyhow::anyhow!("no monitor found"))?;
    let scale = monitor.scale_factor();

    let mon_pos: PhysicalPosition<i32> = *monitor.position();
    let mon_size = monitor.size();

    let mon_x = (mon_pos.x as f64) / scale;
    let mon_y = (mon_pos.y as f64) / scale;
    let mon_w = (mon_size.width as f64) / scale;
    let mon_h = (mon_size.height as f64) / scale;

    let main_outer = window.outer_size().unwrap_or_default();
    let main_w = (main_outer.width as f64) / scale;

    let top_inset = if mon_y == 0.0 { 25.0 } else { 0.0 };

    let region_x = (mon_x + main_w) as i32;
    let region_y = (mon_y + top_inset) as i32;
    let region_w = (mon_w - main_w) as i32;
    let region_h = (mon_h - top_inset) as i32;
    Ok(TileRegion {
        x: region_x,
        y: region_y,
        width: region_w.max(200),
        height: region_h.max(200),
    })
}

/// Scan Claude Code's local storage and return all known historical sessions,
/// with user aliases merged in.
#[command]
pub fn list_claude_sessions(state: State<'_, AppState>) -> Result<Vec<ClaudeHistoryEntry>, String> {
    let mut entries = claude_history::list_claude_sessions().map_err(|e| e.to_string())?;
    for entry in &mut entries {
        if let Some(alias) = state.aliases.get(&entry.session_id) {
            entry.alias = Some(alias.value().clone());
        }
    }
    Ok(entries)
}

#[command]
pub fn list_agent_sessions(
    state: State<'_, AppState>,
    agent: String,
) -> Result<Vec<ClaudeHistoryEntry>, String> {
    let mut entries = claude_history::list_agent_sessions(&agent).map_err(|e| e.to_string())?;
    for entry in &mut entries {
        if let Some(alias) = state.aliases.get(&entry.session_id) {
            entry.alias = Some(alias.value().clone());
        }
    }
    Ok(entries)
}

#[command]
pub fn check_hook_config() -> Result<CombinedHookStatus, String> {
    hook_install::check_agent_hooks().map_err(|e| e.to_string())
}

#[command]
pub fn install_claude_hook() -> Result<HookInstallReport, String> {
    hook_install::install_claude_hook().map_err(|e| e.to_string())
}

#[command]
pub fn install_agent_hooks() -> Result<CombinedHookInstallReport, String> {
    hook_install::install_agent_hooks().map_err(|e| e.to_string())
}

#[command]
pub fn get_http_server_status(health: State<'_, HttpServerHealth>) -> HttpServerStatus {
    health.snapshot()
}

fn append_scanned_history(
    state: &AppState,
    sessions: &mut Vec<SessionEntry>,
    seen: &mut HashSet<String>,
    agent: &str,
) {
    let Ok(entries) = claude_history::list_agent_sessions(agent) else {
        return;
    };
    let active_codex_fallback = if agent == "codex" {
        active_codex_fallback_count(sessions)
    } else {
        0
    };

    for (idx, entry) in entries.into_iter().enumerate() {
        if !seen.insert(entry.session_id.clone()) {
            continue;
        }
        if state.is_history_hidden(&entry.session_id) {
            continue;
        }

        let is_active_codex_fallback = agent == "codex" && idx < active_codex_fallback;
        sessions.push(history_entry_to_session(
            state,
            entry,
            is_active_codex_fallback,
        ));
    }
}

fn active_codex_fallback_count(existing: &[SessionEntry]) -> usize {
    let running = running_codex_process_groups();
    if running == 0 {
        return 0;
    }
    let known_active = existing
        .iter()
        .filter(|s| s.agent == "codex" && s.last_event != "sessionend")
        .count();
    running.saturating_sub(known_active)
}

fn running_codex_process_groups() -> usize {
    let Ok(out) = Command::new("ps")
        .args(["-axo", "pgid=,stat=,comm=,args="])
        .output()
    else {
        return 0;
    };
    if !out.status.success() {
        return 0;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    parse_running_codex_process_groups(&text)
}

fn parse_running_codex_process_groups(text: &str) -> usize {
    let mut groups = HashSet::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(pgid) = parts.next() else {
            continue;
        };
        let Some(stat) = parts.next() else {
            continue;
        };
        if stat.contains('T') {
            continue;
        }
        let rest = parts.collect::<Vec<_>>().join(" ");
        if rest.contains("/bin/codex")
            || rest.contains("/codex ")
            || rest.ends_with("/codex")
            || rest.contains("node /opt/homebrew/bin/codex")
            || rest.contains("node /usr/local/bin/codex")
        {
            groups.insert(pgid.to_string());
        }
    }
    groups.len()
}

fn scanned_history_ids() -> Vec<String> {
    let mut ids = Vec::new();
    for agent in ["claude", "codex"] {
        if let Ok(entries) = claude_history::list_agent_sessions(agent) {
            ids.extend(entries.into_iter().map(|entry| entry.session_id));
        }
    }
    ids
}

fn history_entry_to_session(
    state: &AppState,
    entry: ClaudeHistoryEntry,
    active_fallback: bool,
) -> SessionEntry {
    let alias = state
        .aliases
        .get(&entry.session_id)
        .map(|alias| alias.value().clone())
        .or(entry.alias);
    let cwd = if entry.cwd.is_empty() {
        entry.project
    } else {
        entry.cwd
    };

    SessionEntry {
        session_id: entry.session_id,
        agent: entry.agent,
        iterm_session_id: "unknown".to_string(),
        cwd,
        last_event: if active_fallback {
            "sessionstart".to_string()
        } else {
            "sessionend".to_string()
        },
        last_updated: parse_started_at(entry.started_at.as_deref()),
        notification_count: 0,
        alias,
        preview: entry.summary,
    }
}

fn parse_started_at(started_at: Option<&str>) -> DateTime<Utc> {
    started_at
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_process_fallback_counts_running_process_groups() {
        let ps = r#"
17108 T node node /opt/homebrew/bin/codex
17108 T /opt/homebrew/lib/node_modules/@openai/codex/bin/codex codex
65783 S+ node node /opt/homebrew/bin/codex
65783 S+ /opt/homebrew/lib/node_modules/@openai/codex/bin/codex codex
99999 S+ zsh /bin/zsh
"#;

        assert_eq!(parse_running_codex_process_groups(ps), 1);
    }
}

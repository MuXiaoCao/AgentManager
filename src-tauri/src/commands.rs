use tauri::{command, Manager, State};

use crate::claude_history::{self, ClaudeHistoryEntry};
use crate::hook_install::{self, HookInstallReport, HookStatus};
use crate::iterm::{self, ArrangeReport, TileRegion};
use crate::state::{AppState, SessionEntry};

#[command]
pub fn get_sessions(state: State<'_, AppState>) -> Vec<SessionEntry> {
    state.list_sessions()
}

#[command]
pub fn dismiss_session(state: State<'_, AppState>, session_id: String) -> bool {
    state.dismiss(&session_id)
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
    state: State<'_, AppState>,
    session_id: String,
    cwd: Option<String>,
) -> Result<(), String> {
    let effective_cwd = cwd
        .filter(|s| !s.is_empty())
        .or_else(|| {
            state
                .sessions
                .get(&session_id)
                .map(|r| r.value().cwd.clone())
        })
        .unwrap_or_default();
    if effective_cwd.is_empty() {
        return Err(format!("no cwd available for session {session_id}"));
    }
    iterm::reopen_session(&effective_cwd, &session_id)
        .map_err(|e| e.to_string())
}

/// Persist a display alias for `session_id`. This is purely cosmetic —
/// it only affects the card title inside AgentManager, not the iTerm tab.
#[command]
pub fn rename_session(
    state: State<'_, AppState>,
    session_id: String,
    alias: Option<String>,
) {
    state.set_alias(&session_id, alias);
}

#[command]
pub fn jump_to_iterm(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let Some(entry) = state.sessions.get(&session_id).map(|r| r.value().clone()) else {
        return Err(format!("session {session_id} not found"));
    };
    iterm::jump_to(&entry.iterm_session_id).map_err(|e| e.to_string())
}

/// Arrange iTerm windows for all current sessions into a grid on the primary
/// monitor, excluding the main-window strip on the left.
#[command]
pub fn arrange_iterm_windows(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ArrangeReport, String> {
    let sessions = state.list_sessions();
    let iterm_ids: Vec<String> = sessions
        .into_iter()
        .map(|s| s.iterm_session_id)
        .collect();

    let region = compute_region(&app).map_err(|e| e.to_string())?;
    iterm::arrange_windows(&iterm_ids, region).map_err(|e| e.to_string())
}

fn compute_region(app: &tauri::AppHandle) -> anyhow::Result<TileRegion> {
    use tauri::PhysicalPosition;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow::anyhow!("main window missing"))?;
    let monitor = window
        .primary_monitor()?
        .ok_or_else(|| anyhow::anyhow!("no primary monitor"))?;
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

/// Scan Claude Code's local storage and return all known historical sessions.
#[command]
pub fn list_claude_sessions() -> Result<Vec<ClaudeHistoryEntry>, String> {
    claude_history::list_claude_sessions().map_err(|e| e.to_string())
}

#[command]
pub fn check_hook_config() -> Result<HookStatus, String> {
    hook_install::check_claude_hook().map_err(|e| e.to_string())
}

#[command]
pub fn install_claude_hook() -> Result<HookInstallReport, String> {
    hook_install::install_claude_hook().map_err(|e| e.to_string())
}

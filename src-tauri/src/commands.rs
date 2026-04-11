use serde::Serialize;
use tauri::{command, Manager, State};

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

#[derive(Debug, Serialize)]
pub struct RenameReport {
    pub alias_saved: bool,
    pub iterm_renamed: bool,
    pub iterm_error: Option<String>,
}

/// Persist an alias for `session_id` and, if we have a live iTerm session
/// id, push the same name to iTerm so the tab label matches.
#[command]
pub fn rename_session(
    state: State<'_, AppState>,
    session_id: String,
    alias: Option<String>,
) -> RenameReport {
    state.set_alias(&session_id, alias.clone());

    let trimmed: Option<String> = alias
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let iterm_id: Option<String> = state
        .sessions
        .get(&session_id)
        .map(|r| r.value().iterm_session_id.clone())
        .filter(|id| !id.is_empty() && id != "unknown");

    let (iterm_renamed, iterm_error) = match (trimmed, iterm_id) {
        (Some(name), Some(id)) => match iterm::set_session_name(&id, &name) {
            Ok(()) => (true, None),
            Err(e) => {
                log::warn!("iTerm rename failed for {session_id}: {e}");
                (false, Some(e.to_string()))
            }
        },
        _ => (false, None),
    };

    RenameReport {
        alias_saved: true,
        iterm_renamed,
        iterm_error,
    }
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

#[command]
pub fn check_hook_config() -> Result<HookStatus, String> {
    hook_install::check_claude_hook().map_err(|e| e.to_string())
}

#[command]
pub fn install_claude_hook() -> Result<HookInstallReport, String> {
    hook_install::install_claude_hook().map_err(|e| e.to_string())
}

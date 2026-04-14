mod claude_history;
mod commands;
mod hook_install;
mod http_server;
mod iterm;
mod state;

use tauri::{Manager, PhysicalPosition, PhysicalSize};

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let app_state = AppState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::dismiss_session,
            commands::reorder_sessions,
            commands::clear_history,
            commands::delete_session,
            commands::clear_notifications,
            commands::rename_session,
            commands::reopen_session,
            commands::jump_to_iterm,
            commands::arrange_iterm_windows,
            commands::list_claude_sessions,
            commands::check_hook_config,
            commands::install_claude_hook,
        ])
        .setup(move |app| {
            http_server::spawn(app_state.clone(), app.handle().clone());

            if let Some(window) = app.get_webview_window("main") {
                if let Err(err) = dock_main_window_to_left(&window) {
                    log::warn!("failed to dock main window to left: {err}");
                }
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running AgentManager");
}

/// Feature 2: pin the main window to the left edge of the primary monitor and
/// stretch its height to match the usable screen area.
fn dock_main_window_to_left(window: &tauri::WebviewWindow) -> anyhow::Result<()> {
    const DOCK_WIDTH_POINTS: f64 = 420.0;
    const TOP_INSET_POINTS: f64 = 25.0; // rough macOS menu bar

    let monitor = window
        .primary_monitor()?
        .ok_or_else(|| anyhow::anyhow!("no primary monitor"))?;
    let scale = monitor.scale_factor();
    let mon_pos = *monitor.position();
    let mon_size = monitor.size();

    let width_px = (DOCK_WIDTH_POINTS * scale) as u32;
    let top_inset_px = (TOP_INSET_POINTS * scale) as i32;
    let height_px = mon_size.height.saturating_sub(top_inset_px as u32);

    window.set_position(PhysicalPosition {
        x: mon_pos.x,
        y: mon_pos.y + top_inset_px,
    })?;
    window.set_size(PhysicalSize {
        width: width_px,
        height: height_px,
    })?;
    Ok(())
}

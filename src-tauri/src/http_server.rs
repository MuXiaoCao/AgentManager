use axum::{extract::State, routing::post, Json, Router};
use std::net::SocketAddr;
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, NotifyPayload};

const LISTEN_ADDR: &str = "127.0.0.1:19280";

/// Start the axum server in a background task. Emits `session-updated` events
/// to the frontend whenever an incoming hook notification mutates state.
pub fn spawn(app_state: AppState, app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let router = Router::new()
            .route("/api/notify", post(notify_handler))
            .with_state(Ctx {
                app_state,
                app_handle,
            });

        let addr: SocketAddr = LISTEN_ADDR.parse().expect("valid listen addr");
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => {
                log::info!("Dashboard HTTP server listening on http://{}", addr);
                if let Err(err) = axum::serve(listener, router).await {
                    log::error!("http server exited: {err}");
                }
            }
            Err(err) => {
                log::error!("failed to bind {addr}: {err}");
            }
        }
    });
}

#[derive(Clone)]
struct Ctx {
    app_state: AppState,
    app_handle: AppHandle,
}

async fn notify_handler(
    State(ctx): State<Ctx>,
    Json(payload): Json<NotifyPayload>,
) -> &'static str {
    let entry = ctx.app_state.upsert_from_notify(payload);
    // Notify frontend so it can refresh without polling.
    let _ = ctx.app_handle.emit("session-updated", &entry);
    "ok"
}

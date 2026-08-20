use axum::{extract::State, routing::post, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, NotifyPayload};

const LISTEN_ADDR: &str = "127.0.0.1:19280";
const RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Serialize)]
pub struct HttpServerStatus {
    pub healthy: bool,
    pub retry_count: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Default)]
pub struct HttpServerHealth {
    inner: Arc<Mutex<HttpServerStatus>>,
}

impl Default for HttpServerStatus {
    fn default() -> Self {
        Self {
            healthy: false,
            retry_count: 0,
            last_error: None,
        }
    }
}

impl HttpServerHealth {
    pub fn snapshot(&self) -> HttpServerStatus {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn mark_listening(&self) {
        let mut status = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        status.healthy = true;
        status.last_error = None;
    }

    fn mark_failed(&self, error: String) {
        let mut status = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        status.healthy = false;
        status.retry_count = status.retry_count.saturating_add(1);
        status.last_error = Some(error);
    }
}

/// Start the axum server in a background task. Emits `session-updated` events
/// to the frontend whenever an incoming hook notification mutates state.
pub fn spawn(app_state: AppState, app_handle: AppHandle, health: HttpServerHealth) {
    tauri::async_runtime::spawn(async move {
        let addr: SocketAddr = LISTEN_ADDR.parse().expect("valid listen addr");
        loop {
            let router = Router::new()
                .route("/api/notify", post(notify_handler))
                .with_state(Ctx {
                    app_state: app_state.clone(),
                    app_handle: app_handle.clone(),
                });

            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => {
                    health.mark_listening();
                    log::info!("Dashboard HTTP server listening on http://{}", addr);
                    let error = match axum::serve(listener, router).await {
                        Ok(()) => "http server exited unexpectedly".to_string(),
                        Err(err) => format!("http server exited: {err}"),
                    };
                    log::error!("{error}; retrying in {}s", RETRY_DELAY.as_secs());
                    health.mark_failed(error);
                }
                Err(err) => {
                    let error = format!("failed to bind {addr}: {err}");
                    log::error!("{error}; retrying in {}s", RETRY_DELAY.as_secs());
                    health.mark_failed(error);
                }
            }

            tokio::time::sleep(RETRY_DELAY).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_tracks_failure_and_recovery() {
        let health = HttpServerHealth::default();
        assert!(!health.snapshot().healthy);

        health.mark_failed("bind failed".to_string());
        let failed = health.snapshot();
        assert!(!failed.healthy);
        assert_eq!(failed.retry_count, 1);
        assert_eq!(failed.last_error.as_deref(), Some("bind failed"));

        health.mark_listening();
        let recovered = health.snapshot();
        assert!(recovered.healthy);
        assert_eq!(recovered.retry_count, 1);
        assert!(recovered.last_error.is_none());
    }
}

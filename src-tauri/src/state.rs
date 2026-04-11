use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One Claude Code session tracked by AgentManager. Keyed by `session_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub session_id: String,
    pub agent: String,
    pub cwd: String,
    pub iterm_session_id: String,
    pub last_event: String,
    pub last_updated: DateTime<Utc>,
    pub notification_count: u32,
    /// User-assigned alias (persisted separately); overrides display title when present.
    pub alias: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<DashMap<String, SessionEntry>>,
    pub aliases: Arc<DashMap<String, String>>,
}

impl AppState {
    pub fn new() -> Self {
        let this = Self {
            sessions: Arc::new(DashMap::new()),
            aliases: Arc::new(DashMap::new()),
        };
        this.load_aliases();
        this
    }

    pub fn list_sessions(&self) -> Vec<SessionEntry> {
        let mut out: Vec<SessionEntry> = self
            .sessions
            .iter()
            .map(|r| {
                let mut entry = r.value().clone();
                if let Some(alias) = self.aliases.get(&entry.session_id) {
                    entry.alias = Some(alias.value().clone());
                }
                entry
            })
            .collect();
        // Newest first
        out.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
        out
    }

    pub fn upsert_from_notify(&self, payload: NotifyPayload) -> SessionEntry {
        let now = Utc::now();
        let entry = self
            .sessions
            .entry(payload.session_id.clone())
            .and_modify(|e| {
                e.cwd = payload.cwd.clone();
                e.iterm_session_id = payload.iterm_session_id.clone();
                e.last_event = payload.event_type.clone();
                e.last_updated = now;
                if payload.event_type == "notification" {
                    e.notification_count = e.notification_count.saturating_add(1);
                }
            })
            .or_insert_with(|| SessionEntry {
                session_id: payload.session_id.clone(),
                agent: payload.agent.clone(),
                cwd: payload.cwd.clone(),
                iterm_session_id: payload.iterm_session_id.clone(),
                last_event: payload.event_type.clone(),
                last_updated: now,
                notification_count: if payload.event_type == "notification" { 1 } else { 0 },
                alias: None,
            })
            .clone();
        entry
    }

    pub fn dismiss(&self, session_id: &str) -> bool {
        self.sessions.remove(session_id).is_some()
    }

    pub fn set_alias(&self, session_id: &str, alias: Option<String>) {
        match alias {
            Some(name) if !name.trim().is_empty() => {
                self.aliases
                    .insert(session_id.to_string(), name.trim().to_string());
            }
            _ => {
                self.aliases.remove(session_id);
            }
        }
        let _ = self.save_aliases();
    }

    fn aliases_path() -> Option<std::path::PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("agent-manager");
        p.push("aliases.json");
        Some(p)
    }

    fn load_aliases(&self) {
        let Some(path) = Self::aliases_path() else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&text)
        else {
            return;
        };
        for (k, v) in map {
            if let Some(s) = v.as_str() {
                self.aliases.insert(k, s.to_string());
            }
        }
    }

    fn save_aliases(&self) -> anyhow::Result<()> {
        let Some(path) = Self::aliases_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let map: serde_json::Map<String, serde_json::Value> = self
            .aliases
            .iter()
            .map(|r| (r.key().clone(), serde_json::Value::String(r.value().clone())))
            .collect();
        std::fs::write(&path, serde_json::to_string_pretty(&map)?)?;
        Ok(())
    }
}

/// Payload accepted by POST /api/notify. Mirrors the shape emitted by hook.sh.
#[derive(Debug, Clone, Deserialize)]
pub struct NotifyPayload {
    pub session_id: String,
    pub cwd: String,
    #[serde(default)]
    pub iterm_session_id: String,
    pub event_type: String,
    pub agent: String,
}

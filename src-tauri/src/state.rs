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
    /// Maximum number of sessions persisted to disk. Older entries are pruned
    /// on every save so the history file stays bounded.
    const MAX_HISTORY: usize = 200;

    pub fn new() -> Self {
        let this = Self {
            sessions: Arc::new(DashMap::new()),
            aliases: Arc::new(DashMap::new()),
        };
        this.load_sessions();
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

        // A fresh SessionStart in a given iTerm terminal replaces whatever
        // was previously running there: one shell can only host one Claude
        // process at a time, so any pre-existing card for the same
        // iterm_session_id is stale (the old session either exited cleanly
        // or was killed without firing SessionEnd). Drop those cards so
        // worktree users don't pile up duplicates on the same pane.
        //
        // We only dedupe on SessionStart (not stop/notification/sessionend)
        // to avoid accidentally removing legitimate sibling sessions on
        // intermediate events.
        if payload.event_type == "sessionstart"
            && !payload.iterm_session_id.is_empty()
            && payload.iterm_session_id != "unknown"
        {
            let iterm_id = &payload.iterm_session_id;
            let new_session_id = &payload.session_id;
            let stale: Vec<String> = self
                .sessions
                .iter()
                .filter(|r| {
                    let e = r.value();
                    e.iterm_session_id == *iterm_id && e.session_id != *new_session_id
                })
                .map(|r| r.key().clone())
                .collect();
            if !stale.is_empty() {
                for sid in &stale {
                    self.sessions.remove(sid);
                    self.aliases.remove(sid);
                }
                let _ = self.save_aliases();
            }
        }

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
                } else {
                    // Any non-notification event (stop, userpromptsubmit, etc.)
                    // means the user has moved past the pending notifications.
                    e.notification_count = 0;
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
        let _ = self.save_sessions();
        entry
    }

    pub fn dismiss(&self, session_id: &str) -> bool {
        let removed = self.sessions.remove(session_id).is_some();
        if removed {
            let _ = self.save_sessions();
        }
        removed
    }

    /// Permanently delete a session from history (persisted).
    pub fn delete_session(&self, session_id: &str) -> bool {
        let removed = self.sessions.remove(session_id).is_some();
        self.aliases.remove(session_id);
        if removed {
            let _ = self.save_sessions();
            let _ = self.save_aliases();
        }
        removed
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

    // ── session persistence ────────────────────────────────────────────

    fn sessions_path() -> Option<std::path::PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("agent-manager");
        p.push("sessions.json");
        Some(p)
    }

    fn load_sessions(&self) {
        let Some(path) = Self::sessions_path() else { return };
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let Ok(entries) = serde_json::from_str::<Vec<SessionEntry>>(&text) else {
            return;
        };
        for entry in entries {
            self.sessions.insert(entry.session_id.clone(), entry);
        }
    }

    fn save_sessions(&self) -> anyhow::Result<()> {
        let Some(path) = Self::sessions_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Collect and sort newest-first, then truncate to MAX_HISTORY.
        let mut entries: Vec<SessionEntry> = self
            .sessions
            .iter()
            .map(|r| r.value().clone())
            .collect();
        entries.sort_by(|a, b| b.last_updated.cmp(&a.last_updated));
        entries.truncate(Self::MAX_HISTORY);
        std::fs::write(&path, serde_json::to_string_pretty(&entries)?)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(sid: &str, iterm: &str, event: &str, cwd: &str) -> NotifyPayload {
        NotifyPayload {
            session_id: sid.to_string(),
            cwd: cwd.to_string(),
            iterm_session_id: iterm.to_string(),
            event_type: event.to_string(),
            agent: "claude".to_string(),
        }
    }

    #[test]
    fn sessionstart_in_same_iterm_replaces_old_card() {
        let state = AppState::new();
        state.upsert_from_notify(payload("A", "iterm1", "sessionstart", "/w/worktree-a"));
        // Same iterm_session_id, different session → A should be evicted.
        state.upsert_from_notify(payload("B", "iterm1", "sessionstart", "/w/worktree-b"));

        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 1, "worktree switch should dedupe");
        assert_eq!(sessions[0].session_id, "B");
        assert_eq!(sessions[0].cwd, "/w/worktree-b");
    }

    #[test]
    fn sessionstart_in_different_iterms_does_not_dedupe() {
        let state = AppState::new();
        state.upsert_from_notify(payload("A", "iterm1", "sessionstart", "/w/a"));
        state.upsert_from_notify(payload("B", "iterm2", "sessionstart", "/w/b"));

        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn intermediate_stop_event_does_not_dedupe() {
        let state = AppState::new();
        state.upsert_from_notify(payload("A", "iterm1", "sessionstart", "/w/a"));
        // A hypothetical sibling in the same pane with a non-start event
        // should not cause A to be removed.
        state.upsert_from_notify(payload("B", "iterm1", "stop", "/w/b"));

        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn unknown_iterm_id_does_not_dedupe() {
        let state = AppState::new();
        state.upsert_from_notify(payload("A", "unknown", "sessionstart", "/w/a"));
        state.upsert_from_notify(payload("B", "unknown", "sessionstart", "/w/b"));

        // Both should exist; "unknown" means we can't claim they share a pane.
        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 2);
    }
}

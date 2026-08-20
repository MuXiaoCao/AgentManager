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
    /// Preview of recent activity (last assistant message or user prompt).
    #[serde(default)]
    pub preview: String,
}

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<DashMap<String, SessionEntry>>,
    pub aliases: Arc<DashMap<String, String>>,
    hidden_history: Arc<DashMap<String, ()>>,
}

impl AppState {
    /// Maximum number of sessions persisted to disk. Older entries are pruned
    /// on every save so the history file stays bounded.
    const MAX_HISTORY: usize = 200;

    pub fn new() -> Self {
        let this = Self {
            sessions: Arc::new(DashMap::new()),
            aliases: Arc::new(DashMap::new()),
            hidden_history: Arc::new(DashMap::new()),
        };
        this.load_sessions();
        this.load_aliases();
        this.load_hidden_history();
        this
    }

    #[cfg(test)]
    fn new_empty_for_test() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            aliases: Arc::new(DashMap::new()),
            hidden_history: Arc::new(DashMap::new()),
        }
    }

    pub fn list_sessions(&self) -> Vec<SessionEntry> {
        let order = self.load_order();
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
        // Sort: explicitly ordered sessions first (preserving drag order),
        // then unordered ones by last_updated descending.
        out.sort_by(|a, b| {
            let ai = order.iter().position(|id| id == &a.session_id);
            let bi = order.iter().position(|id| id == &b.session_id);
            match (ai, bi) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.last_updated.cmp(&a.last_updated),
            }
        });
        out
    }

    pub fn reorder_sessions(&self, order: &[String]) {
        if let Some(path) = Self::order_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, serde_json::to_string(order).unwrap_or_default());
        }
    }

    fn order_path() -> Option<std::path::PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("agent-manager");
        p.push("order.json");
        Some(p)
    }

    fn load_order(&self) -> Vec<String> {
        Self::order_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn upsert_from_notify(&self, payload: NotifyPayload) -> SessionEntry {
        let now = Utc::now();
        if self.hidden_history.remove(&payload.session_id).is_some() {
            let _ = self.save_hidden_history();
        }

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

        // Try to get a preview from the session's JSONL file.
        let preview = Self::read_session_preview(&payload.agent, &payload.session_id, &payload.cwd);

        let entry = self
            .sessions
            .entry(payload.session_id.clone())
            .and_modify(|e| {
                e.cwd = payload.cwd.clone();
                e.iterm_session_id = payload.iterm_session_id.clone();
                e.last_event = payload.event_type.clone();
                e.last_updated = now;
                if !preview.is_empty() {
                    e.preview = preview.clone();
                }
                if payload.event_type == "notification" {
                    e.notification_count = e.notification_count.saturating_add(1);
                } else {
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
                notification_count: if payload.event_type == "notification" {
                    1
                } else {
                    0
                },
                alias: None,
                preview: preview.clone(),
            })
            .clone();
        let _ = self.save_sessions();
        entry
    }

    pub fn dismiss(&self, session_id: &str) -> bool {
        let removed = self.sessions.remove(session_id).is_some();
        // A Codex fallback card may be synthesized from history at read time,
        // so it does not necessarily exist in `sessions`. Always persist the
        // hidden marker, even when there was no in-memory entry to remove.
        // A later real hook event clears this marker in upsert_from_notify.
        self.hidden_history.insert(session_id.to_string(), ());
        if removed {
            let _ = self.save_sessions();
        }
        let _ = self.save_hidden_history();
        removed
    }

    /// Clear all ended sessions from history, keeping active ones.
    pub fn clear_history(&self) {
        let ended: Vec<String> = self
            .sessions
            .iter()
            .filter(|r| r.value().last_event == "sessionend")
            .map(|r| r.key().clone())
            .collect();
        for sid in &ended {
            self.sessions.remove(sid);
            self.aliases.remove(sid);
            self.hidden_history.insert(sid.clone(), ());
        }
        if !ended.is_empty() {
            let _ = self.save_sessions();
            let _ = self.save_aliases();
            let _ = self.save_hidden_history();
        }
    }

    /// Permanently delete a session from history (persisted).
    pub fn delete_session(&self, session_id: &str) -> bool {
        let removed = self.sessions.remove(session_id).is_some();
        self.aliases.remove(session_id);
        self.hidden_history.insert(session_id.to_string(), ());
        if removed {
            let _ = self.save_sessions();
        }
        let _ = self.save_aliases();
        let _ = self.save_hidden_history();
        removed
    }

    pub fn is_history_hidden(&self, session_id: &str) -> bool {
        self.hidden_history.contains_key(session_id)
    }

    pub fn hide_history_sessions(&self, session_ids: &[String]) {
        if session_ids.is_empty() {
            return;
        }
        for session_id in session_ids {
            self.hidden_history.insert(session_id.clone(), ());
        }
        let _ = self.save_hidden_history();
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
        let Some(path) = Self::aliases_path() else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
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
            .map(|r| {
                (
                    r.key().clone(),
                    serde_json::Value::String(r.value().clone()),
                )
            })
            .collect();
        std::fs::write(&path, serde_json::to_string_pretty(&map)?)?;
        Ok(())
    }

    fn hidden_history_path() -> Option<std::path::PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("agent-manager");
        p.push("hidden_history.json");
        Some(p)
    }

    fn load_hidden_history(&self) {
        let Some(path) = Self::hidden_history_path() else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(ids) = serde_json::from_str::<Vec<String>>(&text) else {
            return;
        };
        for id in ids {
            self.hidden_history.insert(id, ());
        }
    }

    fn save_hidden_history(&self) -> anyhow::Result<()> {
        let Some(path) = Self::hidden_history_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut ids: Vec<String> = self
            .hidden_history
            .iter()
            .map(|r| r.key().clone())
            .collect();
        ids.sort();
        std::fs::write(&path, serde_json::to_string_pretty(&ids)?)?;
        Ok(())
    }

    // ── session persistence ────────────────────────────────────────────

    /// Read a preview of recent activity from the session's JSONL file.
    /// Searches ALL project dirs for the session_id (because the project
    /// directory encoding doesn't always match the session's cwd).
    fn read_session_preview(agent: &str, session_id: &str, cwd: &str) -> String {
        if agent == "codex" {
            return Self::read_codex_session_preview(session_id);
        }
        Self::read_claude_session_preview(session_id, cwd)
    }

    fn read_claude_session_preview(session_id: &str, _cwd: &str) -> String {
        use std::io::{Read, Seek, SeekFrom};

        let Some(home) = dirs::home_dir() else {
            return String::new();
        };
        let projects_dir = home.join(".claude").join("projects");
        let filename = format!("{}.jsonl", session_id);

        // Search all project directories for this session's JSONL.
        let jsonl_path = std::fs::read_dir(&projects_dir).ok().and_then(|entries| {
            entries.flatten().find_map(|e| {
                let p = e.path().join(&filename);
                if p.exists() {
                    Some(p)
                } else {
                    None
                }
            })
        });

        let Some(jsonl_path) = jsonl_path else {
            return String::new();
        };

        let Ok(mut file) = std::fs::File::open(&jsonl_path) else {
            return String::new();
        };
        let Ok(meta) = file.metadata() else {
            return String::new();
        };
        let file_size = meta.len();

        // Read the last 32KB.
        let read_from = if file_size > 32768 {
            file_size - 32768
        } else {
            0
        };
        let _ = file.seek(SeekFrom::Start(read_from));
        let mut buf = String::new();
        let _ = file.read_to_string(&mut buf);

        // Find the last user or assistant message with text content.
        let mut last_preview = String::new();
        for line in buf.lines().rev() {
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let msg_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if msg_type != "user" && msg_type != "assistant" {
                continue;
            }
            // Extract text from message.content
            let content = obj.pointer("/message/content");
            let text = match content {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(arr)) => arr
                    .iter()
                    .filter_map(|c| {
                        if c.get("type")?.as_str()? == "text" {
                            Some(c.get("text")?.as_str()?.to_string())
                        } else {
                            None
                        }
                    })
                    .next()
                    .unwrap_or_default(),
                _ => continue,
            };
            let trimmed = text.trim();
            if trimmed.len() > 5 {
                let max_len = 200;
                last_preview = if trimmed.len() > max_len {
                    format!("{}…", &trimmed[..trimmed.floor_char_boundary(max_len)])
                } else {
                    trimmed.to_string()
                };
                break;
            }
        }
        last_preview
    }

    fn read_codex_session_preview(session_id: &str) -> String {
        use std::io::{Read, Seek, SeekFrom};

        let Some(home) = dirs::home_dir() else {
            return String::new();
        };
        let sessions_dir = home.join(".codex").join("sessions");
        let Some(jsonl_path) = Self::find_codex_session_file(&sessions_dir, session_id) else {
            return String::new();
        };

        let Ok(mut file) = std::fs::File::open(&jsonl_path) else {
            return String::new();
        };
        let Ok(meta) = file.metadata() else {
            return String::new();
        };
        let file_size = meta.len();
        let read_from = file_size.saturating_sub(32768);
        let _ = file.seek(SeekFrom::Start(read_from));
        let mut buf = String::new();
        let _ = file.read_to_string(&mut buf);

        for line in buf.lines().rev() {
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let text = match obj.get("type").and_then(|v| v.as_str()) {
                Some("event_msg") => {
                    let payload = obj.get("payload");
                    let kind = payload
                        .and_then(|p| p.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if kind != "user_message" && kind != "agent_message" {
                        continue;
                    }
                    payload
                        .and_then(|p| p.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string()
                }
                Some("response_item") => {
                    let payload = obj.get("payload");
                    if payload.and_then(|p| p.get("type")).and_then(|v| v.as_str())
                        != Some("message")
                    {
                        continue;
                    }
                    payload
                        .and_then(|p| p.get("content"))
                        .and_then(|v| v.as_array())
                        .and_then(|arr| {
                            arr.iter().find_map(|item| {
                                if item.get("type")?.as_str()? == "output_text" {
                                    item.get("text")?.as_str()
                                } else {
                                    None
                                }
                            })
                        })
                        .unwrap_or("")
                        .to_string()
                }
                _ => continue,
            };
            let trimmed = text.trim();
            if trimmed.len() > 5 {
                let max_len = 200;
                return if trimmed.len() > max_len {
                    format!("{}…", &trimmed[..trimmed.floor_char_boundary(max_len)])
                } else {
                    trimmed.to_string()
                };
            }
        }
        String::new()
    }

    fn find_codex_session_file(
        dir: &std::path::Path,
        session_id: &str,
    ) -> Option<std::path::PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = Self::find_codex_session_file(&path, session_id) {
                    return Some(found);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.contains(session_id) {
                    return Some(path);
                }
            }
        }
        None
    }

    pub fn save_sessions_pub(&self) -> anyhow::Result<()> {
        self.save_sessions()
    }

    fn sessions_path() -> Option<std::path::PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("agent-manager");
        p.push("sessions.json");
        Some(p)
    }

    fn load_sessions(&self) {
        let Some(path) = Self::sessions_path() else {
            return;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
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
        let mut entries: Vec<SessionEntry> =
            self.sessions.iter().map(|r| r.value().clone()).collect();
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
        let state = AppState::new_empty_for_test();
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
        let state = AppState::new_empty_for_test();
        state.upsert_from_notify(payload("A", "iterm1", "sessionstart", "/w/a"));
        state.upsert_from_notify(payload("B", "iterm2", "sessionstart", "/w/b"));

        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn intermediate_stop_event_does_not_dedupe() {
        let state = AppState::new_empty_for_test();
        state.upsert_from_notify(payload("A", "iterm1", "sessionstart", "/w/a"));
        // A hypothetical sibling in the same pane with a non-start event
        // should not cause A to be removed.
        state.upsert_from_notify(payload("B", "iterm1", "stop", "/w/b"));

        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn unknown_iterm_id_does_not_dedupe() {
        let state = AppState::new_empty_for_test();
        state.upsert_from_notify(payload("A", "unknown", "sessionstart", "/w/a"));
        state.upsert_from_notify(payload("B", "unknown", "sessionstart", "/w/b"));

        // Both should exist; "unknown" means we can't claim they share a pane.
        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn sessionend_moves_active_session_out_of_active_state() {
        let state = AppState::new_empty_for_test();
        state.upsert_from_notify(payload("A", "iterm1", "sessionstart", "/w/a"));
        state.upsert_from_notify(payload("A", "iterm1", "sessionend", "/w/a"));

        let sessions = state.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].last_event, "sessionend");
    }
}

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A session discovered by scanning Claude Code's local storage.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeHistoryEntry {
    pub agent: String,
    pub session_id: String,
    pub cwd: String,
    /// Decoded from the project directory name (e.g. `-Users-xiaocao-foo` → `/Users/xiaocao/foo`).
    pub project: String,
    pub started_at: Option<String>,
    pub kind: String,
    /// First user prompt, truncated. Empty if we couldn't read the JSONL.
    pub summary: String,
    /// Conversation file size in bytes (0 if not found).
    pub size_bytes: u64,
    /// User-assigned alias from AgentManager (persisted in aliases.json).
    pub alias: Option<String>,
}

/// Metadata from `~/.claude/sessions/<pid>.json`.
#[derive(Deserialize)]
struct SessionMeta {
    #[serde(rename = "sessionId")]
    session_id: String,
    cwd: String,
    #[serde(rename = "startedAt", default)]
    started_at: Option<u64>,
    #[serde(default)]
    kind: String,
}

#[derive(Deserialize)]
struct CodexSessionMetaPayload {
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    originator: String,
}

#[derive(Deserialize)]
struct CodexSessionMeta {
    #[serde(default)]
    r#type: String,
    payload: CodexSessionMetaPayload,
}

fn claude_dir() -> Option<PathBuf> {
    let mut p = dirs::home_dir()?;
    p.push(".claude");
    Some(p)
}

fn codex_dir() -> Option<PathBuf> {
    let mut p = dirs::home_dir()?;
    p.push(".codex");
    Some(p)
}

/// Decode the project directory name back to a filesystem path.
/// `-Users-xiaocao-IdeaProjects-foo` → `/Users/xiaocao/IdeaProjects/foo`
fn decode_project_dir(name: &str) -> String {
    if name.starts_with('-') {
        format!("/{}", name[1..].replace('-', "/"))
    } else {
        name.replace('-', "/")
    }
}

fn ts_to_iso(ms: u64) -> String {
    Utc.timestamp_millis_opt(ms as i64)
        .single()
        .map(|dt: DateTime<Utc>| dt.to_rfc3339())
        .unwrap_or_default()
}

fn unix_to_iso(seconds: i64) -> String {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .map(|dt: DateTime<Utc>| dt.to_rfc3339())
        .unwrap_or_default()
}

fn truncate_text(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    let mut s: String = trimmed.chars().take(max_len).collect();
    if trimmed.chars().count() > max_len {
        s.push('…');
    }
    s
}

/// Read the first user prompt from a JSONL conversation file (truncated to
/// `max_len` characters). Reads at most the first 64 KB to stay fast on
/// multi-megabyte logs.
fn extract_summary(path: &std::path::Path, max_len: usize) -> String {
    use std::io::{BufRead, BufReader};
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let reader = BufReader::new(file);
    let mut bytes_read: usize = 0;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        bytes_read += line.len();
        if bytes_read > 64 * 1024 {
            break;
        }
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if obj.get("type").and_then(|v| v.as_str()) != Some("user") {
            continue;
        }
        // message.content is either a string or an array of {type, text} blocks.
        let content = obj.pointer("/message/content").cloned().unwrap_or_default();
        let text = match content {
            serde_json::Value::String(s) => s,
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|c| {
                    if c.get("type")?.as_str()? == "text" {
                        c.get("text")?.as_str().map(String::from)
                    } else {
                        None
                    }
                })
                .next()
                .unwrap_or_default(),
            _ => String::new(),
        };
        let trimmed = text.trim();
        if trimmed.len() > 5 {
            return truncate_text(trimmed, max_len);
        }
    }
    String::new()
}

fn extract_codex_text(obj: &serde_json::Value) -> Option<String> {
    match obj.get("type").and_then(|v| v.as_str()) {
        Some("event_msg") => {
            let payload = obj.get("payload")?;
            match payload.get("type").and_then(|v| v.as_str()) {
                Some("user_message") | Some("agent_message") => payload
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                _ => None,
            }
        }
        Some("response_item") => {
            let payload = obj.get("payload")?;
            if payload.get("type").and_then(|v| v.as_str()) != Some("message") {
                return None;
            }
            let content = payload.get("content")?.as_array()?;
            content.iter().find_map(|item| {
                if item.get("type")?.as_str()? == "output_text" {
                    item.get("text")?.as_str().map(String::from)
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

fn extract_codex_summary(path: &Path, max_len: usize) -> String {
    use std::io::{BufRead, BufReader};
    let Ok(file) = std::fs::File::open(path) else {
        return String::new();
    };
    let reader = BufReader::new(file);
    let mut bytes_read: usize = 0;
    for line in reader.lines() {
        let Ok(line) = line else { break };
        bytes_read += line.len();
        if bytes_read > 128 * 1024 {
            break;
        }
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let Some(text) = extract_codex_text(&obj) else {
            continue;
        };
        let trimmed = text.trim();
        if trimmed.len() > 5 {
            return truncate_text(trimmed, max_len);
        }
    }
    String::new()
}

fn parse_codex_meta(path: &Path) -> Option<CodexSessionMetaPayload> {
    use std::io::{BufRead, BufReader};
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines().take(16) {
        let Ok(line) = line else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<CodexSessionMeta>(&line) else {
            continue;
        };
        if meta.r#type == "session_meta" {
            return Some(meta.payload);
        }
    }
    None
}

fn codex_session_id_from_filename(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    if stem.len() >= 36 {
        stem[stem.len() - 36..].to_string()
    } else {
        stem.to_string()
    }
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

/// Scan Claude Code's local storage and return a list of all known sessions,
/// newest first. Merges metadata from `~/.claude/sessions/` with conversation
/// files found under `~/.claude/projects/`.
pub fn list_claude_sessions() -> Result<Vec<ClaudeHistoryEntry>> {
    let base = claude_dir().unwrap_or_default();
    let sessions_dir = base.join("sessions");
    let projects_dir = base.join("projects");

    // Phase 1: read lightweight metadata from sessions/*.json
    let mut by_id: HashMap<String, ClaudeHistoryEntry> = HashMap::new();
    if let Ok(dir) = std::fs::read_dir(&sessions_dir) {
        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<SessionMeta>(&text) else {
                continue;
            };
            by_id.insert(
                meta.session_id.clone(),
                ClaudeHistoryEntry {
                    agent: "claude".to_string(),
                    session_id: meta.session_id,
                    cwd: meta.cwd,
                    project: String::new(),
                    started_at: meta.started_at.map(ts_to_iso),
                    kind: if meta.kind.is_empty() {
                        "interactive".into()
                    } else {
                        meta.kind
                    },
                    summary: String::new(),
                    size_bytes: 0,
                    alias: None,
                },
            );
        }
    }

    // Phase 2: walk projects/<dir>/<session>.jsonl for conversation files.
    if let Ok(proj_entries) = std::fs::read_dir(&projects_dir) {
        for proj_entry in proj_entries.flatten() {
            let proj_path = proj_entry.path();
            if !proj_path.is_dir() {
                continue;
            }
            let proj_name = proj_entry.file_name().to_string_lossy().into_owned();
            // Skip subagent dirs
            if proj_name == "subagents" {
                continue;
            }
            let project = decode_project_dir(&proj_name);

            if let Ok(files) = std::fs::read_dir(&proj_path) {
                for file_entry in files.flatten() {
                    let fpath = file_entry.path();
                    if fpath.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                        continue;
                    }
                    let sid = fpath
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    // Skip non-UUID-like filenames (e.g. directories)
                    if sid.len() < 8 {
                        continue;
                    }

                    let size = std::fs::metadata(&fpath).map(|m| m.len()).unwrap_or(0);

                    let entry = by_id
                        .entry(sid.clone())
                        .or_insert_with(|| ClaudeHistoryEntry {
                            agent: "claude".to_string(),
                            session_id: sid.clone(),
                            cwd: project.clone(),
                            project: project.clone(),
                            started_at: None,
                            kind: "interactive".into(),
                            summary: String::new(),
                            size_bytes: 0,
                            alias: None,
                        });
                    entry.project = project.clone();
                    entry.size_bytes = size;

                    // Extract summary from first user message (fast: reads ≤64KB).
                    if entry.summary.is_empty() {
                        entry.summary = extract_summary(&fpath, 120);
                    }
                }
            }
        }
    }

    let mut results: Vec<ClaudeHistoryEntry> = by_id.into_values().collect();
    results.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(results)
}

pub fn list_codex_sessions() -> Result<Vec<ClaudeHistoryEntry>> {
    let base = codex_dir().unwrap_or_default();
    let sessions_dir = base.join("sessions");
    let history_path = base.join("history.jsonl");

    let mut files = vec![];
    collect_jsonl_files(&sessions_dir, &mut files);

    let mut by_id: HashMap<String, ClaudeHistoryEntry> = HashMap::new();
    for path in files {
        let meta = parse_codex_meta(&path);
        let sid = meta
            .as_ref()
            .map(|m| {
                if !m.session_id.is_empty() {
                    m.session_id.clone()
                } else {
                    m.id.clone()
                }
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| codex_session_id_from_filename(&path));
        if sid.len() < 8 {
            continue;
        }
        let cwd = meta
            .as_ref()
            .map(|m| m.cwd.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_default();
        let started_at = meta.as_ref().and_then(|m| m.timestamp.clone()).or_else(|| {
            std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| unix_to_iso(d.as_secs() as i64))
        });
        let kind = meta
            .as_ref()
            .map(|m| {
                if !m.source.is_empty() {
                    m.source.clone()
                } else if !m.originator.is_empty() {
                    m.originator.clone()
                } else {
                    "interactive".to_string()
                }
            })
            .unwrap_or_else(|| "interactive".to_string());
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        by_id.insert(
            sid.clone(),
            ClaudeHistoryEntry {
                agent: "codex".to_string(),
                session_id: sid,
                cwd: cwd.clone(),
                project: cwd,
                started_at,
                kind,
                summary: extract_codex_summary(&path, 120),
                size_bytes: size,
                alias: None,
            },
        );
    }

    if let Ok(text) = std::fs::read_to_string(history_path) {
        for line in text.lines() {
            let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(sid) = obj.get("session_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let entry = by_id
                .entry(sid.to_string())
                .or_insert_with(|| ClaudeHistoryEntry {
                    agent: "codex".to_string(),
                    session_id: sid.to_string(),
                    cwd: String::new(),
                    project: String::new(),
                    started_at: obj.get("ts").and_then(|v| v.as_i64()).map(unix_to_iso),
                    kind: "interactive".to_string(),
                    summary: String::new(),
                    size_bytes: 0,
                    alias: None,
                });
            if entry.summary.is_empty() {
                if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                    entry.summary = truncate_text(text, 120);
                }
            }
        }
    }

    let mut results: Vec<ClaudeHistoryEntry> = by_id.into_values().collect();
    results.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    Ok(results)
}

pub fn list_agent_sessions(agent: &str) -> Result<Vec<ClaudeHistoryEntry>> {
    match agent {
        "codex" => list_codex_sessions(),
        _ => list_claude_sessions(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_session_id_fallback_reads_uuid_suffix() {
        let path = Path::new(
            "/tmp/rollout-2026-07-07T00-41-53-019f384e-c152-7212-995a-78562e28f0be.jsonl",
        );
        assert_eq!(
            codex_session_id_from_filename(path),
            "019f384e-c152-7212-995a-78562e28f0be"
        );
    }

    #[test]
    fn codex_summary_reads_user_message() {
        let path = std::env::temp_dir().join(format!(
            "agent-manager-codex-summary-{}.jsonl",
            std::process::id()
        ));
        std::fs::write(
            &path,
            r#"{"timestamp":"2026-07-06T16:42:07.220Z","type":"session_meta","payload":{"session_id":"s1","cwd":"/tmp"}}"#
                .to_string()
                + "\n"
                + r#"{"timestamp":"2026-07-06T16:42:08.220Z","type":"event_msg","payload":{"type":"user_message","message":"hello codex history"}}"#
                + "\n",
        )
        .unwrap();

        assert_eq!(extract_codex_summary(&path, 120), "hello codex history");
        let _ = std::fs::remove_file(path);
    }
}

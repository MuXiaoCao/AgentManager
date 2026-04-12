use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A session discovered by scanning Claude Code's local storage.
#[derive(Debug, Clone, Serialize)]
pub struct ClaudeHistoryEntry {
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

fn claude_dir() -> Option<PathBuf> {
    let mut p = dirs::home_dir()?;
    p.push(".claude");
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
        let content = obj
            .pointer("/message/content")
            .cloned()
            .unwrap_or_default();
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
            let mut s: String = trimmed.chars().take(max_len).collect();
            if trimmed.len() > max_len {
                s.push_str("…");
            }
            return s;
        }
    }
    String::new()
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
            let proj_name = proj_entry
                .file_name()
                .to_string_lossy()
                .into_owned();
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

                    let size = std::fs::metadata(&fpath)
                        .map(|m| m.len())
                        .unwrap_or(0);

                    let entry = by_id.entry(sid.clone()).or_insert_with(|| {
                        ClaudeHistoryEntry {
                            session_id: sid.clone(),
                            cwd: project.clone(),
                            project: project.clone(),
                            started_at: None,
                            kind: "interactive".into(),
                            summary: String::new(),
                            size_bytes: 0,
                            alias: None,
                        }
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

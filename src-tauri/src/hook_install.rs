use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const HOOK_SCRIPT: &str = r#"#!/bin/bash
# AgentManager notification hook for Claude Code.
# Usage: bash ~/.claude-dashboard/hook.sh claude

AGENT="${1:-claude}"

INPUT=$(cat)
EVENT_TYPE=$(echo "$INPUT" | jq -r '.hook_event_name')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id')
CWD=$(echo "$INPUT" | jq -r '.cwd')
STOP_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')

# Avoid re-entering the Stop hook loop.
if [ "$STOP_ACTIVE" = "true" ]; then
  exit 0
fi

ITERM_SID="${ITERM_SESSION_ID:-unknown}"

curl -s -X POST http://127.0.0.1:19280/api/notify \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION_ID\",\"cwd\":\"$CWD\",\"iterm_session_id\":\"$ITERM_SID\",\"event_type\":\"$(echo "$EVENT_TYPE" | tr '[:upper:]' '[:lower:]')\",\"agent\":\"$AGENT\"}" \
  --connect-timeout 1 \
  --max-time 2 \
  > /dev/null 2>&1 || true

exit 0
"#;

/// The three events we care about. SessionStart is the one the original
/// AgentPulse binary forgot, which caused live sessions to only appear after
/// Claude exited.
const EVENTS: &[&str] = &["SessionStart", "Stop", "SessionEnd"];

pub fn hook_script_path() -> Result<PathBuf> {
    let mut p = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    p.push(".claude-dashboard");
    p.push("hook.sh");
    Ok(p)
}

pub fn claude_settings_path() -> Result<PathBuf> {
    let mut p = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    p.push(".claude");
    p.push("settings.json");
    Ok(p)
}

/// Write the bundled hook.sh to `~/.claude-dashboard/hook.sh` if missing or
/// out of date, and chmod it executable.
pub fn ensure_hook_script() -> Result<PathBuf> {
    let script_path = hook_script_path()?;
    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    let needs_write = match fs::read_to_string(&script_path) {
        Ok(existing) => existing != HOOK_SCRIPT,
        Err(_) => true,
    };
    if needs_write {
        fs::write(&script_path, HOOK_SCRIPT)
            .with_context(|| format!("write {}", script_path.display()))?;
    }
    let mut perms = fs::metadata(&script_path)?.permissions();
    if perms.mode() & 0o111 == 0 {
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms)?;
    }
    Ok(script_path)
}

fn build_hook_entry(command: &str) -> Value {
    json!({
        "hooks": [
            { "type": "command", "command": command, "timeout": 5 }
        ]
    })
}

fn entry_contains_command(group: &Value, command: &str) -> bool {
    group
        .pointer("/hooks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().any(|h| {
                h.get("command").and_then(|c| c.as_str()) == Some(command)
            })
        })
        .unwrap_or(false)
}

/// Idempotently add AgentManager's hook under SessionStart / Stop / SessionEnd
/// in `~/.claude/settings.json`. Existing unrelated hooks are preserved.
pub fn install_claude_hook() -> Result<HookInstallReport> {
    let script = ensure_hook_script()?;
    let command = format!("bash {} claude", script.display());

    let settings_path = claude_settings_path()?;
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }

    let mut root: Value = match fs::read_to_string(&settings_path) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str(&text)
            .with_context(|| format!("parse {}", settings_path.display()))?,
        _ => Value::Object(Map::new()),
    };

    if !root.is_object() {
        return Err(anyhow!("{} is not a JSON object", settings_path.display()));
    }

    let hooks = root
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()));
    if !hooks.is_object() {
        return Err(anyhow!("hooks field is not a JSON object"));
    }
    let hooks_map = hooks.as_object_mut().unwrap();

    let mut added = vec![];
    for event in EVENTS {
        let event_arr = hooks_map
            .entry(event.to_string())
            .or_insert_with(|| Value::Array(vec![]));
        if !event_arr.is_array() {
            return Err(anyhow!("hooks.{event} is not a JSON array"));
        }
        let arr = event_arr.as_array_mut().unwrap();
        let already = arr.iter().any(|g| entry_contains_command(g, &command));
        if !already {
            arr.push(build_hook_entry(&command));
            added.push(event.to_string());
        }
    }

    if !added.is_empty() {
        let pretty = serde_json::to_string_pretty(&root)?;
        fs::write(&settings_path, pretty)
            .with_context(|| format!("write {}", settings_path.display()))?;
    }

    Ok(HookInstallReport {
        script_path: script.display().to_string(),
        settings_path: settings_path.display().to_string(),
        added_events: added,
        command,
    })
}

/// Check which of our required hook events are currently installed.
pub fn check_claude_hook() -> Result<HookStatus> {
    let script = hook_script_path()?;
    let command = format!("bash {} claude", script.display());
    let settings_path = claude_settings_path()?;

    let script_installed = script.exists();

    let Ok(text) = fs::read_to_string(&settings_path) else {
        return Ok(HookStatus {
            script_installed,
            settings_exists: false,
            installed_events: vec![],
            expected_command: command,
        });
    };
    let root: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let mut installed = vec![];
    for event in EVENTS {
        let present = root
            .pointer(&format!("/hooks/{}", event))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().any(|g| entry_contains_command(g, &command)))
            .unwrap_or(false);
        if present {
            installed.push(event.to_string());
        }
    }

    Ok(HookStatus {
        script_installed,
        settings_exists: true,
        installed_events: installed,
        expected_command: command,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct HookInstallReport {
    pub script_path: String,
    pub settings_path: String,
    pub added_events: Vec<String>,
    pub command: String,
}

#[derive(Debug, serde::Serialize)]
pub struct HookStatus {
    pub script_installed: bool,
    pub settings_exists: bool,
    pub installed_events: Vec<String>,
    pub expected_command: String,
}

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const HOOK_SCRIPT: &str = r#"#!/bin/bash
# AgentManager notification hook for Claude Code and Codex CLI.
# Usage: bash ~/.claude-dashboard/hook.sh claude|codex

AGENT="${1:-claude}"

INPUT=$(cat)
EVENT_TYPE=$(echo "$INPUT" | jq -r '.hook_event_name // .event // .hook_event // .type // .payload.type // ""')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // .conversation_id // .thread_id // .payload.session_id // .payload.thread_id // empty')
CWD=$(echo "$INPUT" | jq -r '.cwd // .payload.cwd // empty')
STOP_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')

# Avoid re-entering the Stop hook loop.
if [ "$AGENT" = "claude" ] && [ "$STOP_ACTIVE" = "true" ]; then
  exit 0
fi

if [ -z "$SESSION_ID" ] && [ "$AGENT" = "codex" ] && [ -f "$HOME/.codex/history.jsonl" ]; then
  SESSION_ID=$(tail -n 1 "$HOME/.codex/history.jsonl" | jq -r '.session_id // empty' 2>/dev/null || true)
fi

if [ -z "$CWD" ]; then
  CWD="$PWD"
fi

ITERM_SID="${ITERM_SESSION_ID:-unknown}"

EVENT_TYPE=$(echo "$EVENT_TYPE" | tr '[:upper:]' '[:lower:]')
if [ "$AGENT" = "codex" ] && [ "$EVENT_TYPE" = "permissionrequest" ]; then
  EVENT_TYPE="notification"
fi

if [ -z "$SESSION_ID" ] || [ -z "$EVENT_TYPE" ]; then
  exit 0
fi

PAYLOAD=$(jq -n \
  --arg session_id "$SESSION_ID" \
  --arg cwd "$CWD" \
  --arg iterm_session_id "$ITERM_SID" \
  --arg event_type "$EVENT_TYPE" \
  --arg agent "$AGENT" \
  '{session_id:$session_id,cwd:$cwd,iterm_session_id:$iterm_session_id,event_type:$event_type,agent:$agent}')

curl -s -X POST http://127.0.0.1:19280/api/notify \
  -H "Content-Type: application/json" \
  -d "$PAYLOAD" \
  --connect-timeout 1 \
  --max-time 2 \
  > /dev/null 2>&1 || true

exit 0
"#;

/// The five events we track. SessionStart is the one the original AgentPulse
/// binary forgot. UserPromptSubmit lets us show "Working" when Claude is
/// processing. Notification fires when Claude needs user attention (permission
/// prompts, questions, etc.).
const EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "SessionEnd",
];

const CODEX_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "Stop",
    "SessionEnd",
];

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

pub fn codex_hooks_path() -> Result<PathBuf> {
    let mut p = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    p.push(".codex");
    p.push("hooks.json");
    Ok(p)
}

pub fn codex_config_path() -> Result<PathBuf> {
    let mut p = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    p.push(".codex");
    p.push("config.toml");
    Ok(p)
}

/// Write the bundled hook.sh to `~/.claude-dashboard/hook.sh` if missing or
/// out of date, and chmod it executable.
pub fn ensure_hook_script() -> Result<PathBuf> {
    let script_path = hook_script_path()?;
    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
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

fn build_codex_hook_entry(command: &str, matcher: Option<&str>, timeout: u64) -> Value {
    let mut group = Map::new();
    if let Some(matcher) = matcher {
        group.insert("matcher".to_string(), Value::String(matcher.to_string()));
    }
    group.insert(
        "hooks".to_string(),
        json!([
            { "type": "command", "command": command, "timeout": timeout }
        ]),
    );
    Value::Object(group)
}

fn entry_contains_command(group: &Value, command: &str) -> bool {
    group
        .pointer("/hooks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(command))
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
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
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

pub fn install_codex_hook() -> Result<AgentHookInstallReport> {
    let script = ensure_hook_script()?;
    let command = format!("bash {} codex", script.display());
    migrate_codex_hooks_feature()?;

    let hooks_path = codex_hooks_path()?;
    if let Some(parent) = hooks_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let mut root: Value = match fs::read_to_string(&hooks_path) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str(&text)
            .with_context(|| format!("parse {}", hooks_path.display()))?,
        _ => Value::Object(Map::new()),
    };

    if !root.is_object() {
        return Err(anyhow!("{} is not a JSON object", hooks_path.display()));
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
    for event in CODEX_EVENTS {
        let event_arr = hooks_map
            .entry(event.to_string())
            .or_insert_with(|| Value::Array(vec![]));
        if !event_arr.is_array() {
            return Err(anyhow!("hooks.{event} is not a JSON array"));
        }
        let arr = event_arr.as_array_mut().unwrap();
        let already = arr.iter().any(|g| entry_contains_command(g, &command));
        if !already {
            let matcher = match *event {
                "SessionStart" => Some("startup|resume|clear|compact"),
                "PermissionRequest" => Some("*"),
                _ => None,
            };
            let timeout = if *event == "SessionEnd" { 3 } else { 5 };
            arr.push(build_codex_hook_entry(&command, matcher, timeout));
            added.push(event.to_string());
        }
    }

    if !added.is_empty() {
        let pretty = serde_json::to_string_pretty(&root)?;
        fs::write(&hooks_path, pretty)
            .with_context(|| format!("write {}", hooks_path.display()))?;
    }

    Ok(AgentHookInstallReport {
        agent: "codex".to_string(),
        script_path: script.display().to_string(),
        settings_path: hooks_path.display().to_string(),
        added_events: added,
        command,
    })
}

fn migrate_codex_hooks_feature() -> Result<()> {
    let path = codex_config_path()?;
    let Ok(text) = fs::read_to_string(&path) else {
        return Ok(());
    };
    let migrated = migrate_codex_hooks_feature_text(&text);
    if migrated != text {
        fs::write(&path, migrated).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

fn migrate_codex_hooks_feature_text(text: &str) -> String {
    let mut in_features = false;
    text.lines()
        .map(|raw| {
            let code = raw.split('#').next().unwrap_or("").trim();
            if code.starts_with('[') && code.ends_with(']') {
                in_features = code == "[features]";
                return raw.to_string();
            }
            if in_features {
                if let Some((key, _)) = code.split_once('=') {
                    if key.trim() == "codex_hooks" {
                        if let Some(start) = raw.find("codex_hooks") {
                            let mut line = raw.to_string();
                            line.replace_range(start..start + "codex_hooks".len(), "hooks");
                            return line;
                        }
                    }
                }
            }
            raw.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

pub fn install_agent_hooks() -> Result<CombinedHookInstallReport> {
    let claude = install_claude_hook()?;
    let codex = install_codex_hook()?;
    Ok(CombinedHookInstallReport {
        claude: AgentHookInstallReport {
            agent: "claude".to_string(),
            script_path: claude.script_path,
            settings_path: claude.settings_path,
            added_events: claude.added_events,
            command: claude.command,
        },
        codex,
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

pub fn check_codex_hook() -> Result<AgentHookStatus> {
    let script = hook_script_path()?;
    let command = format!("bash {} codex", script.display());
    let hooks_path = codex_hooks_path()?;

    let script_installed = script.exists();
    let hooks_enabled = codex_hooks_enabled();

    let Ok(text) = fs::read_to_string(&hooks_path) else {
        return Ok(AgentHookStatus {
            agent: "codex".to_string(),
            script_installed,
            settings_exists: false,
            installed_events: vec![],
            expected_command: command,
            hooks_enabled,
        });
    };
    let root: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let mut installed = vec![];
    for event in CODEX_EVENTS {
        let present = root
            .pointer(&format!("/hooks/{}", event))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().any(|g| entry_contains_command(g, &command)))
            .unwrap_or(false);
        if present {
            installed.push(event.to_string());
        }
    }

    Ok(AgentHookStatus {
        agent: "codex".to_string(),
        script_installed,
        settings_exists: true,
        installed_events: installed,
        expected_command: command,
        hooks_enabled,
    })
}

pub fn check_agent_hooks() -> Result<CombinedHookStatus> {
    let claude = check_claude_hook()?;
    let codex = check_codex_hook()?;
    Ok(CombinedHookStatus {
        claude: AgentHookStatus {
            agent: "claude".to_string(),
            script_installed: claude.script_installed,
            settings_exists: claude.settings_exists,
            installed_events: claude.installed_events,
            expected_command: claude.expected_command,
            hooks_enabled: true,
        },
        codex,
    })
}

fn codex_hooks_enabled() -> bool {
    let Ok(path) = codex_config_path() else {
        return true;
    };
    let Ok(text) = fs::read_to_string(path) else {
        return true;
    };
    let mut in_features = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_features = line == "[features]";
            continue;
        }
        if in_features {
            let compact = line.replace(' ', "");
            if compact == "hooks=false" || compact == "codex_hooks=false" {
                return false;
            }
        }
    }
    true
}

#[derive(Debug, serde::Serialize)]
pub struct HookInstallReport {
    pub script_path: String,
    pub settings_path: String,
    pub added_events: Vec<String>,
    pub command: String,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentHookInstallReport {
    pub agent: String,
    pub script_path: String,
    pub settings_path: String,
    pub added_events: Vec<String>,
    pub command: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CombinedHookInstallReport {
    pub claude: AgentHookInstallReport,
    pub codex: AgentHookInstallReport,
}

#[derive(Debug, serde::Serialize)]
pub struct HookStatus {
    pub script_installed: bool,
    pub settings_exists: bool,
    pub installed_events: Vec<String>,
    pub expected_command: String,
}

#[derive(Debug, serde::Serialize)]
pub struct AgentHookStatus {
    pub agent: String,
    pub script_installed: bool,
    pub settings_exists: bool,
    pub installed_events: Vec<String>,
    pub expected_command: String,
    pub hooks_enabled: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct CombinedHookStatus {
    pub claude: AgentHookStatus,
    pub codex: AgentHookStatus,
}

#[cfg(test)]
mod tests {
    use super::migrate_codex_hooks_feature_text;

    #[test]
    fn migrates_deprecated_feature_without_touching_other_sections() {
        let input = r#"[features]
codex_hooks = true # keep comment

[hooks.state]
codex_hooks = "unchanged"
"#;
        let expected = r#"[features]
hooks = true # keep comment

[hooks.state]
codex_hooks = "unchanged"
"#;
        assert_eq!(migrate_codex_hooks_feature_text(input), expected);
    }

    #[test]
    fn canonical_feature_is_unchanged() {
        let input = "[features]\nhooks = false\n";
        assert_eq!(migrate_codex_hooks_feature_text(input), input);
    }
}

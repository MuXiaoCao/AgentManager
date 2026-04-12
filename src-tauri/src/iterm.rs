use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::process::Command;

fn run_osascript(script: &str) -> Result<String> {
    let out = Command::new("osascript")
        .args(["-l", "AppleScript", "-e", script])
        .output()
        .context("spawn osascript")?;
    if !out.status.success() {
        return Err(anyhow!(
            "osascript exited with status {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn normalize(raw: &str) -> &str {
    raw.split_once(':').map(|(_, rest)| rest).unwrap_or(raw)
}

fn is_blank(id: &str) -> bool {
    id.is_empty() || id == "unknown"
}

// ── jump ────────────────────────────────────────────────────────────

pub fn jump_to(iterm_session_id: &str) -> Result<()> {
    if is_blank(iterm_session_id) {
        return Err(anyhow!("no iTerm session id recorded"));
    }
    let sid = normalize(iterm_session_id).replace('"', "\\\"");
    let script = format!(
        r#"
tell application "iTerm"
  repeat with w in windows
    repeat with t in tabs of w
      repeat with s in sessions of t
        if unique id of s is "{sid}" then
          select w
          tell t to select
          tell s to select
          return "ok"
        end if
      end repeat
    end repeat
  end repeat
  return "not-found"
end tell
"#,
        sid = sid
    );
    let out = run_osascript(&script)?;
    if out.trim() == "not-found" {
        return Err(anyhow!(
            "iTerm session {iterm_session_id} not found (maybe closed?)"
        ));
    }
    let _ = Command::new("open").args(["-a", "iTerm"]).status();
    Ok(())
}

// ── reopen ──────────────────────────────────────────────────────────

pub fn reopen_session(cwd: &str, claude_session_id: &str) -> Result<()> {
    let safe_cwd = cwd.replace('\'', "'\\''");
    let safe_id = claude_session_id.replace('\'', "'\\''");
    let script = format!(
        r#"
tell application "iTerm"
  create window with default profile
  tell current session of current tab of current window
    write text "cd '{cwd}' && claude --resume '{sid}'"
  end tell
end tell
"#,
        cwd = safe_cwd,
        sid = safe_id,
    );
    run_osascript(&script)?;
    let _ = Command::new("open").args(["-a", "iTerm"]).status();
    Ok(())
}

// ── arrange ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct TileRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

fn resolve_window_ids(sids: &[&str]) -> Result<HashMap<String, i64>> {
    let list_items: Vec<String> = sids
        .iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
        .collect();
    let script = format!(
        r#"
set targetSids to {{{targets}}}
set sidCount to count of targetSids
set outText to ""
repeat with i from 1 to sidCount
  set targetSid to item i of targetSids
  set foundWid to ""
  tell application "iTerm"
    repeat with w in windows
      set wid to id of w
      repeat with t in tabs of w
        repeat with s in sessions of t
          if unique id of s is targetSid then
            set foundWid to wid as string
            exit repeat
          end if
        end repeat
        if foundWid is not "" then exit repeat
      end repeat
      if foundWid is not "" then exit repeat
    end repeat
  end tell
  set outText to outText & targetSid & "=" & foundWid & linefeed
end repeat
return outText
"#,
        targets = list_items.join(", ")
    );
    let out = run_osascript(&script)?;
    let mut map = HashMap::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((sid, wid_str)) = line.split_once('=') {
            if let Ok(wid) = wid_str.trim().parse::<i64>() {
                map.insert(sid.to_string(), wid);
            }
        }
    }
    Ok(map)
}

fn apply_bounds(assignments: &[(i64, i32, i32, i32, i32)]) -> Result<(usize, usize)> {
    if assignments.is_empty() {
        return Ok((0, 0));
    }
    let mut script = String::from("set arranged to 0\nset skipped to 0\n");
    script.push_str("tell application \"iTerm\"\n");
    for &(wid, x1, y1, x2, y2) in assignments {
        script.push_str(&format!(
            "  try\n    set bounds of (first window whose id is {wid}) to {{{x1}, {y1}, {x2}, {y2}}}\n    set arranged to arranged + 1\n  on error\n    set skipped to skipped + 1\n  end try\n"
        ));
    }
    script.push_str("end tell\n");
    script.push_str("return (arranged as string) & \",\" & (skipped as string)\n");
    let out = run_osascript(&script)?;
    Ok(parse_pair(out.trim()))
}

pub fn arrange_windows(session_ids: &[String], region: TileRegion) -> Result<ArrangeReport> {
    let live: Vec<&str> = session_ids
        .iter()
        .filter(|s| !is_blank(s))
        .map(|s| normalize(s.as_str()))
        .collect();

    let blank_count = session_ids.len() - live.len();

    if live.is_empty() {
        return Ok(ArrangeReport {
            arranged: 0,
            skipped: blank_count,
            cols: 0,
            rows: 0,
        });
    }

    let sid_to_wid = resolve_window_ids(&live)?;
    let missing_count = live.len() - sid_to_wid.len();

    let mut unique_wids: Vec<i64> = Vec::new();
    for sid in &live {
        if let Some(&wid) = sid_to_wid.get(*sid) {
            if !unique_wids.contains(&wid) {
                unique_wids.push(wid);
            }
        }
    }

    let n = unique_wids.len() as i32;
    if n == 0 {
        return Ok(ArrangeReport {
            arranged: 0,
            skipped: blank_count + missing_count,
            cols: 0,
            rows: 0,
        });
    }

    let cols = ((n as f64).sqrt().ceil()) as i32;
    let rows = ((n as f64 / cols as f64).ceil()) as i32;
    let cell_w = (region.width / cols).max(1);
    let cell_h = (region.height / rows).max(1);

    let mut assignments = Vec::with_capacity(n as usize);
    for (i, &wid) in unique_wids.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        let x1 = region.x + col * cell_w;
        let y1 = region.y + row * cell_h;
        assignments.push((wid, x1, y1, x1 + cell_w, y1 + cell_h));
    }

    let (arranged, apply_skipped) = apply_bounds(&assignments)?;

    Ok(ArrangeReport {
        arranged,
        skipped: blank_count + missing_count + apply_skipped,
        cols: cols as usize,
        rows: rows as usize,
    })
}

fn parse_pair(s: &str) -> (usize, usize) {
    let mut it = s.split(',');
    let a = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
    let b = it.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
    (a, b)
}

#[derive(Debug, serde::Serialize)]
pub struct ArrangeReport {
    pub arranged: usize,
    pub skipped: usize,
    pub cols: usize,
    pub rows: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_iterm_prefix() {
        assert_eq!(normalize("w0t0p0:F103A515"), "F103A515");
        assert_eq!(normalize("w12t3p4:AD7F157A"), "AD7F157A");
    }

    #[test]
    fn normalize_passes_through_plain_uuid() {
        assert_eq!(normalize("264E7062"), "264E7062");
    }

    #[test]
    fn normalize_handles_empty_and_unknown() {
        assert!(is_blank(""));
        assert!(is_blank("unknown"));
        assert!(!is_blank("w0t0p0:abc"));
    }

    #[test]
    fn parse_pair_handles_expected_format() {
        assert_eq!(parse_pair("3,0"), (3, 0));
        assert_eq!(parse_pair("0,2"), (0, 2));
        assert_eq!(parse_pair(""), (0, 0));
    }
}

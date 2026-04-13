use anyhow::{anyhow, Context, Result};
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

/// Set position + size for ALL iTerm windows using System Events
/// (Accessibility API). Unlike `tell application "iTerm"`, System Events
/// does NOT trigger the Dock's "switch to app's Space" behavior, so windows
/// stay on the user's current desktop.
fn apply_bounds_via_system_events(region: &TileRegion) -> Result<(usize, usize)> {
    // The grid computation happens inside AppleScript so we only need one
    // osascript call. We pass the region as parameters.
    let script = format!(
        r#"
set regionX to {rx}
set regionY to {ry}
set regionW to {rw}
set regionH to {rh}

tell application "System Events"
  tell process "iTerm2"
    set winList to every window
    set n to count of winList
    if n is 0 then return "0,0"

    -- Compute grid: cols = ceil(sqrt(n)), rows = ceil(n / cols)
    set cols to (round ((n ^ 0.5) + 0.4999) rounding up) as integer
    if cols < 1 then set cols to 1
    set rows to (round ((n / cols) + 0.4999) rounding up) as integer
    if rows < 1 then set rows to 1
    set cellW to regionW div cols
    set cellH to regionH div rows

    set arranged to 0
    repeat with i from 1 to n
      set idx to i - 1
      set col to idx mod cols
      set row to idx div cols
      set x1 to regionX + col * cellW
      set y1 to regionY + row * cellH
      try
        set position of window i to {{x1, y1}}
        set size of window i to {{cellW, cellH}}
        set arranged to arranged + 1
      end try
    end repeat

    -- Raise all windows: set frontmost to bring entire app layer forward.
    set frontmost to true
  end tell
end tell
return (arranged as string) & ",0"
"#,
        rx = region.x,
        ry = region.y,
        rw = region.width,
        rh = region.height,
    );
    let out = run_osascript(&script)?;
    Ok(parse_pair(out.trim()))
}

/// Arrange ALL iTerm windows into a grid on the screen area to the right of
/// AgentManager. Uses System Events (Accessibility API) instead of iTerm's
/// own AppleScript to avoid triggering macOS's Dock-based Space switching
/// (`tell application "iTerm"` causes the desktop to jump to iTerm's "home"
/// Space even when both apps are on the same desktop).
pub fn arrange_windows(region: TileRegion) -> Result<ArrangeReport> {
    let (arranged, skipped) = apply_bounds_via_system_events(&region)?;

    // Compute cols/rows for the report (mirror the AppleScript logic).
    let n = arranged + skipped;
    let cols = if n > 0 {
        ((n as f64).sqrt().ceil()) as usize
    } else {
        0
    };
    let rows = if cols > 0 {
        ((n as f64 / cols as f64).ceil()) as usize
    } else {
        0
    };

    Ok(ArrangeReport {
        arranged,
        skipped,
        cols,
        rows,
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

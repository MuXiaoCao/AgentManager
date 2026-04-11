use anyhow::{anyhow, Context, Result};
use std::process::Command;

/// Run an AppleScript snippet with osascript, returning stdout.
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

/// Rename an iTerm session's tab label. iTerm2's `name` property is the
/// string shown in the tab bar; setting it disables the session's auto-name
/// override for the rest of the session.
pub fn set_session_name(iterm_session_id: &str, name: &str) -> Result<()> {
    if iterm_session_id.is_empty() || iterm_session_id == "unknown" {
        return Err(anyhow!("no iTerm session id recorded"));
    }
    let sid = iterm_session_id.replace('"', "\\\"");
    // Escape backslashes first, then quotes, then newlines -> spaces.
    let safe_name = name
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', " ");
    let script = format!(
        r#"
tell application "iTerm"
  repeat with w in windows
    repeat with t in tabs of w
      repeat with s in sessions of t
        if unique id of s is "{sid}" then
          set name of s to "{name}"
          return "ok"
        end if
      end repeat
    end repeat
  end repeat
  return "not-found"
end tell
"#,
        sid = sid,
        name = safe_name
    );
    let out = run_osascript(&script)?;
    if out.trim() == "not-found" {
        return Err(anyhow!(
            "iTerm session {iterm_session_id} not found (maybe closed?)"
        ));
    }
    Ok(())
}

/// Activate iTerm and focus the tab/session whose session id matches.
/// `iTerm2` AppleScript exposes session ids that match $ITERM_SESSION_ID.
pub fn jump_to(iterm_session_id: &str) -> Result<()> {
    if iterm_session_id.is_empty() || iterm_session_id == "unknown" {
        return Err(anyhow!("no iTerm session id recorded"));
    }
    // escape quotes just in case
    let sid = iterm_session_id.replace('"', "\\\"");
    let script = format!(
        r#"
tell application "iTerm"
  activate
  repeat with w in windows
    repeat with t in tabs of w
      repeat with s in sessions of t
        if unique id of s is "{sid}" then
          select w
          tell w to select t
          tell t to select s
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
    Ok(())
}

/// Screen rect reserved for the tiled iTerm windows (main window's area is excluded).
#[derive(Debug, Clone, Copy)]
pub struct TileRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Re-arrange the iTerm windows belonging to `session_ids` into a grid.
/// The grid is laid out as `cols × rows` where `cols = ceil(sqrt(n))`.
/// Sessions whose iTerm window can't be found are skipped silently.
pub fn arrange_windows(session_ids: &[String], region: TileRegion) -> Result<ArrangeReport> {
    // Filter out empty/placeholder ids up front.
    let live_ids: Vec<&str> = session_ids
        .iter()
        .map(|s| s.as_str())
        .filter(|s| !s.is_empty() && *s != "unknown")
        .collect();
    if live_ids.is_empty() {
        return Ok(ArrangeReport {
            arranged: 0,
            skipped: session_ids.len(),
            cols: 0,
            rows: 0,
        });
    }

    let n = live_ids.len() as i32;
    let cols = ((n as f64).sqrt().ceil()) as i32;
    let rows = ((n as f64 / cols as f64).ceil()) as i32;
    let cell_w = region.width / cols;
    let cell_h = region.height / rows;

    // Build AppleScript: one pass that knows about every session id and the
    // bounds to assign. We encode the (id, bounds) list as a parallel list
    // of AppleScript records.
    let mut script = String::new();
    script.push_str("set assignments to {");
    for (idx, sid) in live_ids.iter().enumerate() {
        let col = (idx as i32) % cols;
        let row = (idx as i32) / cols;
        let x1 = region.x + col * cell_w;
        let y1 = region.y + row * cell_h;
        let x2 = x1 + cell_w;
        let y2 = y1 + cell_h;
        if idx > 0 {
            script.push_str(", ");
        }
        let safe_sid = sid.replace('"', "\\\"");
        script.push_str(&format!(
            r#"{{sid:"{}", b:{{{}, {}, {}, {}}}}}"#,
            safe_sid, x1, y1, x2, y2
        ));
    }
    script.push_str("}\n");
    script.push_str(
        r#"
set arranged to 0
set skipped to 0
tell application "iTerm"
  activate
  repeat with a in assignments
    set targetSid to sid of a
    set targetBounds to b of a
    set found to false
    repeat with w in windows
      repeat with t in tabs of w
        repeat with s in sessions of t
          if unique id of s is targetSid then
            try
              set bounds of w to targetBounds
              set arranged to arranged + 1
              set found to true
            on error
              set skipped to skipped + 1
            end try
            exit repeat
          end if
        end repeat
        if found then exit repeat
      end repeat
      if found then exit repeat
    end repeat
    if not found then set skipped to skipped + 1
  end repeat
end tell
return (arranged as string) & "," & (skipped as string)
"#,
    );

    let out = run_osascript(&script)?;
    let (arranged, skipped) = parse_arranged_reply(out.trim());
    Ok(ArrangeReport {
        arranged,
        skipped: (skipped as usize)
            + (session_ids.len() - live_ids.len()),
        cols: cols as usize,
        rows: rows as usize,
    })
}

fn parse_arranged_reply(s: &str) -> (usize, usize) {
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

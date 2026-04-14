use anyhow::{anyhow, Context, Result};
use std::process::Command;

/// Run AppleScript by spawning `/usr/bin/osascript`. Suitable for commands
/// that don't need Accessibility permission (e.g. talking to iTerm directly).
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

/// Run AppleScript **in-process** via `NSAppleScript`. This uses the calling
/// app's (AgentManager's) Accessibility permission instead of osascript's,
/// which solves the "osascript不允许辅助访问" (-25211) error for System Events
/// commands that require Accessibility.
#[cfg(target_os = "macos")]
fn run_applescript_inline(source: &str) -> Result<String> {
    use objc::{class, msg_send, sel, sel_impl};
    use objc::runtime::Object;
    use std::ffi::{CStr, CString};

    let source_c = CString::new(source).context("script contains null byte")?;

    unsafe {
        // NSString *src = [NSString stringWithUTF8String:source];
        let src_ns: *mut Object =
            msg_send![class!(NSString), stringWithUTF8String: source_c.as_ptr()];
        if src_ns.is_null() {
            return Err(anyhow!("failed to create NSString from script source"));
        }

        // NSAppleScript *script = [[NSAppleScript alloc] initWithSource:src];
        let script_obj: *mut Object = msg_send![class!(NSAppleScript), alloc];
        let script_obj: *mut Object = msg_send![script_obj, initWithSource: src_ns];
        if script_obj.is_null() {
            return Err(anyhow!("NSAppleScript initWithSource returned nil"));
        }

        // NSDictionary *error = nil;
        // NSAppleEventDescriptor *result = [script executeAndReturnError:&error];
        let mut error_dict: *mut Object = std::ptr::null_mut();
        let result: *mut Object =
            msg_send![script_obj, executeAndReturnError: &mut error_dict];

        if result.is_null() {
            // Extract error message from the NSDictionary.
            let err_msg = if !error_dict.is_null() {
                let key: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: b"NSAppleScriptErrorMessage\0".as_ptr()];
                let msg_ns: *mut Object = msg_send![error_dict, objectForKey: key];
                if !msg_ns.is_null() {
                    let cstr: *const std::os::raw::c_char = msg_send![msg_ns, UTF8String];
                    CStr::from_ptr(cstr).to_string_lossy().into_owned()
                } else {
                    "unknown NSAppleScript error".to_string()
                }
            } else {
                "unknown NSAppleScript error".to_string()
            };
            let _: () = msg_send![script_obj, release];
            return Err(anyhow!("AppleScript error: {}", err_msg));
        }

        // NSString *output = [result stringValue];
        let string_val: *mut Object = msg_send![result, stringValue];
        let output = if !string_val.is_null() {
            let cstr: *const std::os::raw::c_char = msg_send![string_val, UTF8String];
            CStr::from_ptr(cstr).to_string_lossy().into_owned()
        } else {
            String::new()
        };

        let _: () = msg_send![script_obj, release];
        Ok(output)
    }
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
          -- Visual pulse: briefly expand the window then restore,
          -- so the user can immediately spot which window was focused
          -- when multiple windows are tiled side by side.
          try
            set origBounds to bounds of w
            set {{x1, y1, x2, y2}} to origBounds
            set bounds of w to {{x1 - 25, y1 - 25, x2 + 25, y2 + 25}}
            delay 0.15
            set bounds of w to origBounds
          end try
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
/// Check Accessibility permission with caching. The prompt dialog is shown
/// at most ONCE per app session. After the user grants access, subsequent
/// calls return true immediately without any dialog.
#[cfg(target_os = "macos")]
fn ensure_accessibility() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::ffi::c_void;
    use std::ptr;

    static GRANTED: AtomicBool = AtomicBool::new(false);
    static PROMPTED: AtomicBool = AtomicBool::new(false);

    // Fast path: already confirmed.
    if GRANTED.load(Ordering::Relaxed) {
        return true;
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDictionaryCreate(
            alloc: *const c_void, keys: *const *const c_void,
            values: *const *const c_void, count: isize,
            kcb: *const c_void, vcb: *const c_void,
        ) -> *const c_void;
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: *const c_void;
        static kCFBooleanFalse: *const c_void;
        fn CFStringCreateWithCString(
            alloc: *const c_void, cstr: *const u8, enc: u32,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    unsafe {
        // Quick check without prompt first.
        if AXIsProcessTrusted() {
            GRANTED.store(true, Ordering::Relaxed);
            return true;
        }

        // Show the prompt dialog only once per app session.
        if PROMPTED.swap(true, Ordering::Relaxed) {
            return false; // already prompted, don't nag
        }

        let key = CFStringCreateWithCString(
            ptr::null(), b"AXTrustedCheckOptionPrompt\0".as_ptr(), 0x08000100,
        );
        let keys = [key];
        let values = [kCFBooleanTrue];
        let dict = CFDictionaryCreate(
            ptr::null(), keys.as_ptr(), values.as_ptr(), 1,
            &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks,
        );
        let result = AXIsProcessTrustedWithOptions(dict);
        CFRelease(dict);
        CFRelease(key);

        if result {
            GRANTED.store(true, Ordering::Relaxed);
        }
        result
    }
}

fn apply_bounds_via_system_events(region: &TileRegion) -> Result<(usize, usize)> {
    // First, count iTerm windows via a tiny System Events query.
    let count_script = r#"
tell application "System Events"
  return count of windows of process "iTerm2"
end tell
"#;
    let count_out = run_applescript_inline(count_script)?;
    let n: usize = count_out.trim().parse().unwrap_or(0);
    if n == 0 {
        return Ok((0, 0));
    }

    // Compute the grid entirely in Rust — no math in AppleScript.
    let cols = ((n as f64).sqrt().ceil()) as i32;
    let rows = ((n as f64 / cols as f64).ceil()) as i32;
    let cell_w = (region.width / cols).max(1);
    let cell_h = (region.height / rows).max(1);

    // Build a minimal AppleScript that only sets position + size using
    // pre-computed literal values. No math, no reserved-word variables,
    // no round/div/mod — just plain `set position of window i to {x, y}`.
    let mut script = String::new();
    script.push_str("tell application \"System Events\"\n");
    script.push_str("  tell process \"iTerm2\"\n");
    for i in 0..n {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        let x = region.x + col * cell_w;
        let y = region.y + row * cell_h;
        let win_idx = i + 1; // AppleScript windows are 1-indexed
        script.push_str(&format!(
            "    try\n      set position of window {idx} to {{{x}, {y}}}\n      set size of window {idx} to {{{w}, {h}}}\n      perform action \"AXRaise\" of window {idx}\n    end try\n",
            idx = win_idx, x = x, y = y, w = cell_w, h = cell_h
        ));
    }
    // NOTE: intentionally NOT setting frontmost — that triggers Dock's
    // space-switching behavior. AXRaise on each window is enough to
    // bring them above other windows on the current Space.
    script.push_str("  end tell\n");
    script.push_str("end tell\n");
    script.push_str(&format!("return \"{n},0\"\n", n = n));

    let out = run_applescript_inline(&script)?;
    Ok(parse_pair(out.trim()))
}

/// Arrange ALL iTerm windows into a grid on the screen area to the right of
/// AgentManager. Uses System Events (Accessibility API) instead of iTerm's
/// own AppleScript to avoid triggering macOS's Dock-based Space switching
/// (`tell application "iTerm"` causes the desktop to jump to iTerm's "home"
/// Space even when both apps are on the same desktop).
pub fn arrange_windows(region: TileRegion) -> Result<ArrangeReport> {
    if !ensure_accessibility() {
        return Err(anyhow!(
            "需要辅助功能权限。请在 系统设置 → 隐私与安全性 → 辅助功能 中授权 AgentManager，然后重试。"
        ));
    }

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

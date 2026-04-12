#!/bin/bash
# Post-build step: ensure NSAppleEventsUsageDescription is present in the
# bundled Info.plist. Without it, macOS may refuse (or silently drop)
# AppleEvents from AgentManager to iTerm on stricter OS versions.
#
# Run automatically by `npm run tauri:build` and `npm run tauri:dev`.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"

USAGE_STRING="AgentManager focuses and arranges iTerm sessions to match your Claude Code cards."

patch_plist() {
  local plist="$1"
  if [ ! -f "$plist" ]; then
    return 0
  fi
  if /usr/libexec/PlistBuddy -c "Print :NSAppleEventsUsageDescription" "$plist" >/dev/null 2>&1; then
    echo "[patch-plist] already present in $plist"
  else
    plutil -insert NSAppleEventsUsageDescription -string "$USAGE_STRING" "$plist"
    echo "[patch-plist] added NSAppleEventsUsageDescription to $plist"
  fi
}

# Patch every AgentManager.app under target/ so both debug and release
# builds carry the key. Ignore errors if the path doesn't exist yet.
for path in \
  "$ROOT/src-tauri/target/release/bundle/macos/AgentManager.app/Contents/Info.plist" \
  "$ROOT/src-tauri/target/debug/bundle/macos/AgentManager.app/Contents/Info.plist"
do
  patch_plist "$path"
done

# ── Compile and bundle the move-to-space helper ─────────────────────
SWIFT_SRC="$ROOT/scripts/move-to-space.swift"
if [ -f "$SWIFT_SRC" ]; then
  for app_dir in \
    "$ROOT/src-tauri/target/release/bundle/macos/AgentManager.app/Contents/MacOS" \
    "$ROOT/src-tauri/target/debug/bundle/macos/AgentManager.app/Contents/MacOS"
  do
    if [ -d "$app_dir" ]; then
      echo "[patch-plist] compiling move-to-space helper → $app_dir"
      swiftc -O "$SWIFT_SRC" -o "$app_dir/move-to-space" \
        -framework CoreGraphics -framework AppKit 2>&1 || true
    fi
  done
fi

#!/bin/bash
# One-command install: kill → reset TCC → copy → launch.
# Usage: npm run install:app   (or: bash scripts/install.sh)
set -e

APP_SRC="src-tauri/target/release/bundle/macos/AgentManager.app"
APP_DST="/Applications/AgentManager.app"
BUNDLE_ID="com.xiaocao.agentmanager"

if [ ! -d "$APP_SRC" ]; then
  echo "❌ Build first: npm run tauri:build"
  exit 1
fi

echo "⏹  Stopping running instance..."
pgrep -f agent-manager | xargs -r kill -9 2>/dev/null || true
sleep 1

echo "🔑 Resetting Accessibility permission (binary hash changed)..."
tccutil reset Accessibility "$BUNDLE_ID" 2>/dev/null || true

echo "📦 Installing to /Applications..."
rm -rf "$APP_DST"
cp -R "$APP_SRC" "$APP_DST"

echo "🚀 Launching..."
open "$APP_DST"

echo "✅ Done. First 'Arrange' click will prompt for Accessibility — grant it once."

#!/bin/bash
# Build bin/Reminders Bridge.app: the EventKit helper as a background app with a stable
# identity (local.reminders-mcp.bridge), so the Reminders permission is granted to it once.
set -euo pipefail
cd "$(dirname "$0")/.."
APP="bin/Reminders Bridge.app"
rm -rf "$APP" bin/reminders-helper
mkdir -p "$APP/Contents/MacOS"
cp helper/Info.plist "$APP/Contents/Info.plist"
swiftc -O helper/main.swift -o "$APP/Contents/MacOS/reminders-helper"
codesign --force --sign - --identifier local.reminders-mcp.bridge "$APP"
echo "Built: $APP"

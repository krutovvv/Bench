#!/bin/bash
# Hand a change to the person: build THIS commit's app and open it next to the one they run.
# Run from the project folder (where scripts/bundle-macos.sh is), after committing.
#
# Usage: ship.sh [--no-open]
#
# Bench is developed from inside Bench: the app the person works in (and talks to you through)
# is never quit or replaced. scripts/bundle-macos.sh builds "dist/Bench.<commit>.app" as a new
# app; this opens it beside the running one. The person tries it, and when it's good they move
# the Dock icon to it themselves; the previous builds stay for rolling back (the newest 6).
# Nothing here kills a process.
set -euo pipefail
[[ -x scripts/bundle-macos.sh ]] || { echo "run from the project folder" >&2; exit 1; }
OUT=$(scripts/bundle-macos.sh 2>&1) || { printf '%s\n' "$OUT" >&2; exit 1; }
printf '%s\n' "$OUT" | grep -E '^(Built|removed|kept)' || true
APP=$(printf '%s\n' "$OUT" | sed -n 's/^Built: //p' | tail -1)
[[ -n "$APP" && -d "$APP" ]] || { echo "no app was built" >&2; exit 1; }
if [[ "${1:-}" != "--no-open" ]]; then
    open "$APP"
    echo "opened next to the running one: $APP"
fi

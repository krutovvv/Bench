#!/bin/bash
# Hand a change to the person. Run from the project folder, after committing.
#
# Usage: ship.sh [--no-open]
#
# There is one app: Bench (`/Applications/Bench.app`) — the person works in it and talks to you
# through it. It is only a client: the threads, the queue and the agent live in the agent process
# and in ~/.bench, so restarting the app costs nothing and loses nothing, and the way back from a
# bad change is an earlier commit (the tree must be clean to build), not an older copy of the
# app. ship.sh therefore quits Bench, rebuilds it in place and opens it again.
set -euo pipefail
[[ -x scripts/bundle-macos.sh ]] || { echo "run from the project folder" >&2; exit 1; }
OPEN=1
for a in "$@"; do
    case "$a" in
    --no-open) OPEN=0 ;;
    *) echo "usage: ship.sh [--no-open]" >&2; exit 1 ;;
    esac
done

# A running app is quit the honest way (never killed): its unfinished run goes on in the agent
# and the window comes back to it.
RUNNING="/Applications/Bench.app/Contents/MacOS/"; NAME="Bench"; ID="local.bench.app"
# Is it running? pgrep cannot be trusted from the agent's shell (it does not see the app at
# all, so a quit was once skipped and the person kept running the previous build), so ask ps.
running() {
    # Only a process whose program is the app's: awk's own arguments hold the same path, and a
    # match anywhere on the line found awk itself, so a Bench that had quit read as still running.
    ps -ww -eo pid=,args= 2>/dev/null | awk -v pat="$RUNNING" \
        'index($2, pat) == 1 { found = 1 } END { exit !found }'
}
if running; then
    osascript -e "tell application id \"$ID\" to quit" >/dev/null 2>&1 || true
    # Up to a minute: a Bench busy painting a long thread can take more than twenty seconds.
    for _ in $(seq 120); do
        running || break
        sleep 0.5
    done
    if running; then
        echo "$NAME didn't quit — ask the person to quit it, then ship again" >&2
        exit 1
    fi
fi

OUT=$(scripts/bundle-macos.sh 2>&1) || { printf '%s\n' "$OUT" >&2; exit 1; }
printf '%s\n' "$OUT" | grep -E '^(Built|previous|no Apple)' || true
APP=$(printf '%s\n' "$OUT" | sed -n 's/^Built: \(.*\) (.*)$/\1/p' | tail -1)
[[ -n "$APP" && -d "$APP" ]] || { echo "no app was built" >&2; exit 1; }

if [[ "$OPEN" == 1 ]]; then
    open "$APP"
    echo "opened: $APP"
else
    echo "built, not opened: $APP"
fi

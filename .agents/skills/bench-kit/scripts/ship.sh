#!/bin/bash
# Hand a change to the person. Run from the project folder, after committing.
#
# Usage: ship.sh [--dev] [--no-open]
#
# There is one working app: Bench (`/Applications/Bench.app`) — the person works in it and talks
# to you through it. It is only a client: the threads, the queue and the agent live in the agent
# process and in ~/.bench, so restarting the app costs nothing and loses nothing, and the way
# back from a bad change is an earlier commit (the tree must be clean to build), not an older
# copy of the app. ship.sh therefore quits Bench, rebuilds it in place and opens it again.
#
# `--dev` builds Bench Dev (`dist/Bench Dev.app`, bundle id local.bench.dev) beside it, for the
# rare case of comparing two states of the interface. Both apps use the same ~/.bench — the same
# agent, the same databases — so Dev isolates nothing but a window.
set -euo pipefail
[[ -x scripts/bundle-macos.sh ]] || { echo "run from the project folder" >&2; exit 1; }
KIND=release OPEN=1
for a in "$@"; do
    case "$a" in
    --dev) KIND=dev ;;
    --release) KIND=release ;;
    --no-open) OPEN=0 ;;
    *) echo "usage: ship.sh [--dev] [--no-open]" >&2; exit 1 ;;
    esac
done

# A running app is quit the honest way (never killed): its unfinished run goes on in the agent
# and the window comes back to it.
if [[ "$KIND" == dev ]]; then
    RUNNING="/dist/Bench Dev.app/Contents/MacOS/"; NAME="Bench Dev"; ID="local.bench.dev"
else
    RUNNING="/Applications/Bench.app/Contents/MacOS/"; NAME="Bench"; ID="local.bench.app"
fi
# Is it running? pgrep cannot be trusted from the agent's shell (it does not see the app at
# all, so a quit was once skipped and the person kept running the previous build), so ask ps.
running() {
    ps -ww -eo pid=,args= 2>/dev/null | awk -v pat="$RUNNING" \
        'index($0, pat) { found = 1 } END { exit !found }'
}
if running; then
    osascript -e "tell application id \"$ID\" to quit" >/dev/null 2>&1 || true
    # A minute, not twenty seconds: a Bench busy painting a long thread was still on its way out
    # after twenty, ship.sh gave up, and the app was left half-quit — the agent it started loses
    # the local network then, so every model call failed until the person quit it by hand.
    for _ in $(seq 120); do
        running || break
        sleep 0.5
    done
    if running; then
        echo "$NAME didn't quit — ask the person to quit it, then ship again" >&2
        exit 1
    fi
fi

OUT=$(scripts/bundle-macos.sh "$KIND" 2>&1) || { printf '%s\n' "$OUT" >&2; exit 1; }
printf '%s\n' "$OUT" | grep -E '^(Built|previous|no Apple)' || true
APP=$(printf '%s\n' "$OUT" | sed -n 's/^Built: \(.*\) (.*)$/\1/p' | tail -1)
[[ -n "$APP" && -d "$APP" ]] || { echo "no app was built" >&2; exit 1; }

if [[ "$OPEN" == 1 ]]; then
    open "$APP"
    echo "opened: $APP"
else
    echo "built, not opened: $APP"
fi

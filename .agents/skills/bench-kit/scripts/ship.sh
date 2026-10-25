#!/bin/bash
# Hand a change to the person. Run from the project folder, after committing.
#
# Usage: ship.sh [--release] [--no-open]
#
# Bench is developed from inside Bench, which has two apps (scripts/bundle-macos.sh):
#   * Bench Dev ("dist/Bench Dev.app"): every commit is tried here. ship.sh asks a running
#     Bench Dev to quit (a normal quit, never a kill), rebuilds it in place and opens it.
#   * Bench ("~/Applications/Bench.app"): the one the person works in and talks to you through.
#     Only `ship.sh --release`, when the person asks for it, rebuilds it; it never quits it: the
#     running one keeps going from Bench.prev.app and the new one starts on the next open.
set -euo pipefail
[[ -x scripts/bundle-macos.sh ]] || { echo "run from the project folder" >&2; exit 1; }
KIND=dev OPEN=1
for a in "$@"; do
    case "$a" in
    --release) KIND=release ;;
    --no-open) OPEN=0 ;;
    *) echo "usage: ship.sh [--release] [--no-open]" >&2; exit 1 ;;
    esac
done

if [[ "$KIND" == dev ]]; then
    # By the end of its path: Bench Dev may have been started through a link or an older path.
    RUNNING="/dist/Bench Dev.app/Contents/MacOS/"
    if pgrep -f "$RUNNING" >/dev/null; then
        osascript -e 'tell application id "local.bench.dev" to quit' >/dev/null 2>&1 || true
        for _ in $(seq 20); do
            pgrep -f "$RUNNING" >/dev/null || break
            sleep 0.5
        done
        if pgrep -f "$RUNNING" >/dev/null; then
            echo "Bench Dev didn't quit — ask the person to quit it, then ship again" >&2
            exit 1
        fi
    fi
fi

OUT=$(scripts/bundle-macos.sh "$KIND" 2>&1) || { printf '%s\n' "$OUT" >&2; exit 1; }
printf '%s\n' "$OUT" | grep -E '^(Built|previous|no Apple)' || true
APP=$(printf '%s\n' "$OUT" | sed -n 's/^Built: \(.*\) (.*)$/\1/p' | tail -1)
[[ -n "$APP" && -d "$APP" ]] || { echo "no app was built" >&2; exit 1; }

if [[ "$KIND" == dev && "$OPEN" == 1 ]]; then
    open "$APP"
    echo "opened Bench Dev: $APP"
elif [[ "$KIND" == release ]]; then
    echo "released: the next time Bench is opened it is this commit (the running one goes on until then)"
fi

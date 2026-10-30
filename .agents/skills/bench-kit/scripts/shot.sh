#!/bin/bash
# Screenshot YOUR OWN copy of the app (never the person's window), then quit that copy.
#
# Usage:
#   shot.sh <binary | App.app> <out.png> [seconds] [KEY=VALUE ...]
#
#   binary      ./target/debug/<crate> (or target/release/...) — fast, but no macOS permissions
#               (the microphone is refused for a bare binary)
#   App.app     dist/<Title>.app — launched as a separate instance with `open -n`, so it has the
#               permissions the person granted to the app
#   seconds     how long to wait before the shot (default 10; ~12 for a chat on This Mac,
#               25–40 when the app connects over SSH)
#   KEY=VALUE   environment for this copy only, e.g. BENCH_DEMO=landing (the table of hooks:
#               references/build-verification.md)
#
# Prints the PNG's size in px and in pt (Retina: 2 px = 1 pt). Exit 0 = a fresh picture exists;
# exit 1 = no picture (and no old one left behind to be measured by mistake). Our copy is always
# quit, whatever happens.
set -uo pipefail
TARGET=$1
OUT=$2
WAIT=${3:-10}
shift $(( $# >= 3 ? 3 : $# ))
DIR=$(cd "$(dirname "$0")" && pwd)

rm -f "$OUT"
P=""
cleanup() {
    if [[ -n "$P" ]]; then
        kill "$P" 2>/dev/null
        wait "$P" 2>/dev/null   # no "Terminated: 15" line
    fi
    return 0
}
trap cleanup EXIT

if [[ "$TARGET" == *.app || "$TARGET" == *.app/ ]]; then
    # The full path: every Bench is "Bench.app", so the name alone would match the person's.
    APPDIR=$(cd "${TARGET%/}" && pwd)
    ENVS=()
    for kv in "$@"; do ENVS+=(--env "$kv"); done
    BEFORE=$(pgrep -f "$APPDIR/Contents/MacOS/")
    open -n "$APPDIR" ${ENVS[@]+"${ENVS[@]}"}
    sleep 1
    # Our copy = the process from that bundle that wasn't running before (the person's copy of
    # the same bundle, if they run it, stays alone).
    for pid in $(pgrep -f "$APPDIR/Contents/MacOS/"); do
        grep -qx "$pid" <<<"$BEFORE" || P=$pid
    done
    [[ -n "$P" ]] || { echo "could not find the new instance" >&2; exit 1; }
else
    env "$@" "$TARGET" >/tmp/app-shot.log 2>&1 &
    P=$!
fi

sleep "$WAIT"
# The window may still be appearing or not yet drawable: try a few times, 2 s apart.
for _ in 1 2 3 4 5; do
    ID=$(swift "$DIR/winid.swift" "$P" 2>/dev/null)
    if [[ -n "$ID" ]] && screencapture -x -o -l "$ID" "$OUT" 2>/dev/null && [[ -s "$OUT" ]]; then
        break
    fi
    rm -f "$OUT"
    sleep 2
done
if [[ ! -s "$OUT" ]]; then
    echo "no screenshot: no window for pid $P (crashed? see /tmp/app-shot.log; slow? wait longer)" >&2
    exit 1
fi
python3 -I -c 'import sys; from PIL import Image; w, h = Image.open(sys.argv[1]).size; print(f"{sys.argv[1]}: {w}x{h} px = {w//2}x{h//2} pt")' "$OUT"
exit 0

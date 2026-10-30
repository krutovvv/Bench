#!/bin/bash
# shoot.sh <dir> [name-regex]: every state from a copy of target/debug/bench, one at a time.
# The copy is removed first: cp over a binary still running breaks its signature on macOS.
HERE=$(cd "$(dirname "$0")" && pwd); W=${PIXELS_DIR:-/tmp/bench-pixels}; mkdir -p "$W"
OUT=$W/$1; F=${2:-[a-z0-9]+}; rm -rf "$OUT"; mkdir -p "$OUT"
[ -x "$W/winid" ] || swiftc -O -o "$W/winid" "$HERE/../winid.swift"
BIN=$W/bin-$1; rm -f "$BIN"; cp "$HERE/../../../../../target/debug/bench" "$BIN"
grep -E "^($F) " "$HERE/states.txt" | while read name envs; do "$HERE/one.sh" "$BIN" "$OUT" "$name" $envs </dev/null; done
rm -f "$BIN"; echo done

#!/bin/bash
# one.sh <bin> <outdir> <name> [ENV=VAL...]: a copy started, shot when its window is full size, quit.
W=$(cd "$(dirname "$0")" && pwd)
BIN=$1; OUT=$2; n=$3; shift 3
ID_BIN=${PIXELS_DIR:-/tmp/bench-pixels}/winid
for try in 1 2 3; do
  env "$@" "$BIN" >"$OUT/$n.log" 2>&1 &
  P=$!
  sleep 8
  for i in 1 2 3 4 5 6; do
    ID=$("$ID_BIN" $P 2>/dev/null)
    if [ -n "$ID" ] && [ "$ID" != 0 ] && screencapture -x -o -l "$ID" "$OUT/$n.png" 2>/dev/null && python3 -I -c 'import sys;from PIL import Image;sys.exit(0 if Image.open(sys.argv[1]).size[0]>=2000 else 1)' "$OUT/$n.png" 2>/dev/null; then break; fi
    rm -f "$OUT/$n.png"; sleep 1
  done
  kill $P 2>/dev/null; wait $P 2>/dev/null
  [ -s "$OUT/$n.png" ] && exit 0
done
echo FAIL $n

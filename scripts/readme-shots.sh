#!/bin/bash
# The README's pictures, each from its own copy of the app on the demo's made-up data, into
# assets/shots/. Run it when what the README shows has changed, then read every picture before
# committing.
#
# macOS only: shot.sh takes the picture with screencapture. A new picture is a line here and a
# line in the README; the hooks are in .agents/skills/bench-kit/references/build-verification.md.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build

shot() {
    local name=$1
    shift
    .agents/skills/bench-kit/scripts/shot.sh target/debug/bench "assets/shots/$name.png" 12 "$@"
    sips --resampleWidth 1600 "assets/shots/$name.png" >/dev/null
}

shot chat BENCH_DEMO=top
shot streams BENCH_DEMO=landing BENCH_PANEL=providers BENCH_GEAR_TAB=0
shot line BENCH_DEMO=landing BENCH_DEMO_LINE=2
shot review BENCH_DEMO=landing BENCH_DEMO_FILES=1 BENCH_DEMO_REVIEW=1
shot editor BENCH_DEMO=explorer BENCH_EXPLORER=thread

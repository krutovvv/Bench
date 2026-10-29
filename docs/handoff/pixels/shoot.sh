#!/bin/bash
# shoot.sh <outdir> [name-filter-regex]: copies target/debug/bench, then shoots each state from the copy.
W=${PIXELS_DIR:-/tmp/bench-pixels}
OUT=$W/$1; F=${2:-[a-z0-9]+}
mkdir -p $OUT
cd /Users/vladimir/Projects/bench
BIN=$W/bin-$1
cp target/debug/bench $BIN
grep -E "^($F) " $W/states.txt | while read name envs; do
  .agents/skills/bench-kit/scripts/shot.sh $BIN $OUT/$name.png 10 $envs </dev/null >/dev/null 2>&1 || echo "FAIL $name"
done
rm -f $BIN
echo done

# Pixel checks

Every demo state in `states.txt`, shot from a copy of `target/debug/bench` and compared against a
reference. Shots want a quiet machine: no `cargo build` at the same time, or windows come up late.

1. On the commit before your change: `cargo build`, then `scripts/pixels/shoot.sh before`.
2. On your change: `cargo build`, then `scripts/pixels/shoot.sh after`.
3. `python3 scripts/pixels/cmp.py after`: `same`, or `DIFF <box> px=<count>`. A narrow 4 by 36 px box
   is the caret blinking; the meters' seconds move too. Look at any other box.

`PIXELS_DIR` (default `/tmp/bench-pixels`) keeps the shots. `shoot.sh <dir> '<regex>'` shoots only the
states whose names match. It needs Pillow.

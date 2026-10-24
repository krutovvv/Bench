"""Where text (or an icon) starts, left to right, in a horizontal band — in points.

Usage:
  python3 -I textx.py <shot.png> <from_x_pt> <y0_pt> <y1_pt> [<y0_pt> <y1_pt> ...]

For each band y0..y1 (pt) prints the first x (pt) right of from_x where a dark pixel appears
(darker than the band's background by a clear margin), and that x minus from_x. Use from_x =
the divider between sidebar and main area to get "inset from the divider"; compare several
bands (the question, the answer, the composer) — insets that must be equal should differ by
≤ 0.5 pt (glyph side bearings), more is a real offset.
"""
import sys

import numpy as np
from PIL import Image

img = np.asarray(Image.open(sys.argv[1]).convert("L")).astype(int)
x0 = int(float(sys.argv[2]) * 2)
args = sys.argv[3:]
for i in range(0, len(args) - 1, 2):
    y0, y1 = int(float(args[i]) * 2), int(float(args[i + 1]) * 2)
    band = img[y0:y1, x0 + 2:]
    bg = int(np.median(band))
    dark = np.where((band < bg - 60).any(axis=0))[0]
    if len(dark) == 0:
        print(f"y {args[i]}–{args[i + 1]} pt: nothing dark found")
        continue
    x = (x0 + 2 + dark[0]) / 2
    print(f"y {args[i]}–{args[i + 1]} pt: starts at x = {x:.1f} pt  (inset {x - x0 / 2:.1f} pt)")

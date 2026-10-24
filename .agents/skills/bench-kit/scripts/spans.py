"""What is drawn along a horizontal band, left to right — icons, words, buttons — in points.

Usage:
  python3 -I spans.py <shot.png> <y0_pt> <y1_pt> [<x0_pt> <x1_pt>]

Looks at the band y0..y1 (pt) between x0 and x1 (default: the whole width), finds the
background (the most common color), and prints each run of "something drawn" as start–end in pt
with its width, plus the gaps between them and to both ends of the range. Gaps under 3 pt inside
one word or icon are merged, so a label reads as one span.

Typical uses:
- icons in a strip: their positions, the gaps between them, the last one's distance to the
  sidebar's right edge (give x1 = the sidebar's width, e.g. 0 280);
- a button's label centered in its bar (gap left ≈ gap right);
- what moved after a change: run it on the before and the after shot, compare.
To find y0..y1: `rules.py` gives the rules; a bar's content is between two of them.
"""
import sys

import numpy as np
from PIL import Image

img = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
H, W = img.shape[:2]
y0, y1 = int(float(sys.argv[2]) * 2), int(float(sys.argv[3]) * 2)
x0 = int(float(sys.argv[4]) * 2) if len(sys.argv) > 4 else 0
x1 = int(float(sys.argv[5]) * 2) if len(sys.argv) > 5 else W
band = img[y0:y1, x0:x1]
flat = band.reshape(-1, 3)
colors, counts = np.unique(flat, axis=0, return_counts=True)
bg = colors[counts.argmax()]
drawn = (np.abs(band - bg).max(axis=2) > 24).any(axis=0)

spans, start = [], None
for i, d in enumerate(list(drawn) + [False]):
    if d and start is None:
        start = i
    elif not d and start is not None:
        spans.append([start, i])
        start = None
merged = []
for s in spans:
    if merged and s[0] - merged[-1][1] < 6:  # < 3 pt: same word or icon
        merged[-1][1] = s[1]
    else:
        merged.append(s)

pt = lambda px: (x0 + px) / 2
print(f"band y {sys.argv[2]}–{sys.argv[3]} pt, x {pt(0):.1f}–{pt(x1 - x0):.1f} pt, background rgb{tuple(bg)}")
prev = 0
for a, b in merged:
    print(f"  gap {(a - prev) / 2:6.1f} pt   then drawn {pt(a):7.1f}–{pt(b):.1f} pt  (width {(b - a) / 2:.1f})")
    prev = b
print(f"  gap {(x1 - x0 - prev) / 2:6.1f} pt to the end of the range")

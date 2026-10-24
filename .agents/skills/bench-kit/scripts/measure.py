"""Pixel ruler for window screenshots (Retina: 2 px = 1 pt).

Usage:
  python3 -I measure.py <shot.png> column <x> <y0> <y1>   # runs of equal color going down
  python3 -I measure.py <shot.png> row <y> <x0> <x1>      # runs of equal color going right

Each run prints: start, length in px, RGB. A 1-2 px run is a border; compare run
lengths (gap above vs. below, left vs. right) instead of guessing from a screenshot.
"""
import sys

import numpy as np
from PIL import Image

img = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
mode, fixed, a, b = sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
line = img[a:b, fixed] if mode == "column" else img[fixed, a:b]
start, prev = a, tuple(line[0])
for i, px in enumerate(line[1:], start=a + 1):
    px = tuple(px)
    if max(abs(p - q) for p, q in zip(px, prev)) > 4:
        print(start, i - start, prev)
        start, prev = i, px
print(start, b - start, prev)

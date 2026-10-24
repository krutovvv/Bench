"""Where the horizontal rules (1px lines) are, going down one column — in points.

Usage:
  python3 -I rules.py <shot.png> <x_pt> [<x_pt> ...]

For each x (in pt, as in the app; the screenshot is Retina, 2 px = 1 pt) prints the y of every
thin line crossing that column and the height of each band between them. Run it on two columns
to compare areas side by side, e.g. the sidebar (x=100) and the main area (x=600): rules that
must be "one line across the window" must have the same y in both lists.

A rule = a uniform run of 1–2 px (0.5–1 pt) that differs from the runs above and below; text
rows don't count (not uniform across the 30 pt segment). An "edge" = where one filled area meets
another (the top of a filled button) — compare it with the rules too.
"""
import sys

import numpy as np
from PIL import Image

img = np.asarray(Image.open(sys.argv[1]).convert("RGB")).astype(int)
H = img.shape[0]


W = img.shape[1]


def row_runs(x):
    """Rows grouped into runs of the same look across a 30 pt segment centred on x."""
    a, b = max(0, x - 30), min(W, x + 30)
    seg = img[:, a:b]
    mean = seg.mean(axis=1)
    uniform = np.abs(seg - mean[:, None, :]).max(axis=(1, 2)) <= 6
    runs = []
    for y in range(H):
        key = (bool(uniform[y]), mean[y])
        if runs and runs[-1][2] == key[0] and (not key[0] or np.abs(runs[-1][3] - key[1]).max() <= 4):
            runs[-1][1] += 1
        else:
            runs.append([y, 1, key[0], key[1]])
    return runs


for arg in sys.argv[2:]:
    x = int(float(arg) * 2)
    rs = row_runs(x)
    print(f"x = {arg} pt")
    prev = 0.0
    for i, (y, n, uni, col) in enumerate(rs):
        if not uni or n > 2 or i == 0 or i == len(rs) - 1:
            continue
        before, after = rs[i - 1], rs[i + 1]
        if np.abs(before[3] - col).max() <= 8 or np.abs(after[3] - col).max() <= 8:
            continue
        top, bottom = y / 2, (y + n) / 2
        print(f"  rule {top:7.1f}–{bottom:.1f} pt   (band above it: {top - prev:6.1f} pt)")
        prev = bottom
    # Edges between two filled areas (a button's top against the sidebar) — no line, still a
    # boundary that must meet the rules across the window.
    for i in range(1, len(rs)):
        (y0, n0, u0, c0), (y1, n1, u1, c1) = rs[i - 1], rs[i]
        if u0 and u1 and n0 >= 8 and n1 >= 8 and np.abs(c0 - c1).max() > 20:
            print(f"  edge at {y1 / 2:7.1f} pt   (a filled area starts or ends)")
    print(f"  band below the last rule: {H / 2 - prev:.1f} pt")

"""Cut one area out of a window screenshot, in points, to look at it closely.

Usage:
  python3 -I crop.py <shot.png> <x_pt> <y_pt> <w_pt> <h_pt> <out.png> [scale]

Coordinates are in pt from the window's top-left (the screenshot is Retina: 2 px = 1 pt).
`scale` (default 1) enlarges the crop for reading small details (e.g. 2 to see 1px steps).
Look at the crop, never at a crop of the whole screen: other windows get into those.
"""
import sys

from PIL import Image

src, x, y, w, h, out = sys.argv[1], *map(float, sys.argv[2:6]), sys.argv[6]
scale = float(sys.argv[7]) if len(sys.argv) > 7 else 1.0
img = Image.open(src)
box = tuple(int(v * 2) for v in (x, y, x + w, y + h))
part = img.crop(box)
if scale != 1.0:
    part = part.resize((int(part.width * scale), int(part.height * scale)), Image.NEAREST)
part.save(out)
print(f"{out}: {part.width}x{part.height} px")

"""Packs the rendered cube frames into the sprite sheet the app plays at startup.

    python tools/intro_sheet.py <frames dir> [preview.png]

Writes src/assets/intro.webp (12 columns x 8 rows, frame 0 top left). With a second argument
also writes a contact sheet on a dark background, tinted green, to look at.
"""
import os
import sys

from PIL import Image

COLS, ROWS = 12, 8
src = sys.argv[1]
names = sorted(n for n in os.listdir(src) if n.endswith(".png"))
assert len(names) == COLS * ROWS, len(names)
frames = [Image.open(os.path.join(src, n)).convert("RGBA") for n in names]
w, h = frames[0].size

sheet = Image.new("RGBA", (COLS * w, ROWS * h), (0, 0, 0, 0))
for i, frame in enumerate(frames):
    sheet.paste(frame, ((i % COLS) * w, (i // COLS) * h))
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "assets", "intro.webp")
sheet.save(out, "WEBP", quality=88, method=6, alpha_quality=95)
print(f"{len(frames)} frames of {w}x{h} -> {os.path.normpath(out)}, {os.path.getsize(out) // 1024} KB")

if len(sys.argv) > 2:
    picks = [0, 4, 8, 10, 12, 16, 20, 24, 36, 48, 60, 72]
    preview = Image.new("RGBA", (len(picks) // 2 * w, 2 * h), (9, 12, 11, 255))
    half = len(picks) // 2
    for n, i in enumerate(picks):
        r, g, b, a = frames[i].split()
        tint = Image.merge("RGBA", (r.point(lambda v: v * 61 // 255), g.point(lambda v: v * 220 // 255), b.point(lambda v: v * 132 // 255), a))
        preview.alpha_composite(tint, ((n % half) * w, (n // half) * h))
    preview.save(sys.argv[2])
    print("preview", sys.argv[2])

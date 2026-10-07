"""Draw the app icon: the 3x3 block from the map, bright centre, dimmer ring.

    python tools/gen_icon.py && npx tauri icon tools/icon.png
"""
import os, struct, zlib

SIZE = 1024
BG = (9, 12, 11)
ACCENT = (61, 220, 132)


def mix(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def main():
    px = [[(0, 0, 0, 0)] * SIZE for _ in range(SIZE)]
    radius = 208

    def inside_round(x, y):
        cx = min(max(x, radius), SIZE - 1 - radius)
        cy = min(max(y, radius), SIZE - 1 - radius)
        return (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2

    for y in range(SIZE):
        row = px[y]
        for x in range(SIZE):
            if inside_round(x, y):
                row[x] = BG + (255,)

    cell, gap = 196, 44
    start = (SIZE - (cell * 3 + gap * 2)) // 2
    for r in range(3):
        for c in range(3):
            centre = r == 1 and c == 1
            edge = (r + c) % 2 == 1
            colour = ACCENT if centre else mix(BG, ACCENT, 0.5 if edge else 0.26)
            x0, y0 = start + c * (cell + gap), start + r * (cell + gap)
            for y in range(y0, y0 + cell):
                row = px[y]
                for x in range(x0, x0 + cell):
                    row[x] = colour + (255,)

    raw = b"".join(b"\x00" + bytes(v for p in row for v in p) for row in px)

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", SIZE, SIZE, 8, 6, 0, 0, 0)) \
        + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    dst = os.path.join(os.path.dirname(os.path.abspath(__file__)), "icon.png")
    open(dst, "wb").write(png)
    print(dst)


if __name__ == "__main__":
    main()

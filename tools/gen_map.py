"""Rasterise Natural Earth country outlines into the dot grid the map draws.

    python tools/gen_map.py

Writes src/assets/world.json: a Mercator grid, one byte per cell (0 = sea, n = countries[n-1]),
and the cell each country is anchored at (where the path ends).
"""
import base64, json, math, os

HERE = os.path.dirname(os.path.abspath(__file__))
COLS = 168
LAT_TOP, LAT_BOTTOM = 78.0, -56.0

# Places Natural Earth 110m leaves out or draws too small to catch a cell.
EXTRA = {
    "SG": (1.35, 103.82), "HK": (22.32, 114.17), "MT": (35.9, 14.45), "BH": (26.07, 50.55),
    "LI": (47.16, 9.55), "MC": (43.74, 7.42), "AD": (42.5, 1.52), "MO": (22.2, 113.55),
    "MU": (-20.2, 57.5), "SC": (-4.68, 55.49), "MV": (3.2, 73.22), "BB": (13.19, -59.54),
    "IM": (54.24, -4.55), "JE": (49.21, -2.13), "GI": (36.14, -5.35), "SM": (43.94, 12.46),
    "LU": (49.61, 6.13), "CY": (35.13, 33.43), "IS": (64.96, -19.02), "TW": (23.7, 121.0),
}
FIX_ISO = {"France": "FR", "Norway": "NO", "Kosovo": "XK", "N. Cyprus": "CY", "Somaliland": "SO"}


def merc(lat):
    return math.log(math.tan(math.pi / 4 + math.radians(lat) / 2))


Y_TOP, Y_BOTTOM = merc(LAT_TOP), merc(LAT_BOTTOM)
CELL = 2 * math.pi / COLS
ROWS = int(round((Y_TOP - Y_BOTTOM) / CELL))


def cell_of(lat, lon):
    col = int((lon + 180.0) / 360.0 * COLS) % COLS
    row = int((Y_TOP - merc(max(min(lat, LAT_TOP - 0.01), LAT_BOTTOM + 0.01))) / CELL)
    return col, min(max(row, 0), ROWS - 1)


def lonlat(col, row, fx=0.5, fy=0.5):
    lon = (col + fx) / COLS * 360.0 - 180.0
    y = Y_TOP - (row + fy) * CELL
    return lon, math.degrees(2 * math.atan(math.exp(y)) - math.pi / 2)


def in_ring(x, y, ring):
    inside = False
    j = len(ring) - 1
    for i in range(len(ring)):
        xi, yi = ring[i][0], ring[i][1]
        xj, yj = ring[j][0], ring[j][1]
        if (yi > y) != (yj > y) and x < (xj - xi) * (y - yi) / (yj - yi) + xi:
            inside = not inside
        j = i
    return inside


def main():
    geo = json.load(open(os.path.join(HERE, "ne_110m_countries.geojson"), encoding="utf-8"))
    countries, shapes, labels = [], [], {}
    for f in geo["features"]:
        p = f["properties"]
        iso = p.get("ISO_A2_EH") or p.get("ISO_A2")
        if not iso or iso == "-99":
            iso = FIX_ISO.get(p.get("NAME"))
        if not iso or iso == "AQ":
            continue
        g = f["geometry"]
        polys = [g["coordinates"]] if g["type"] == "Polygon" else g["coordinates"]
        if iso not in countries:
            countries.append(iso)
        idx = countries.index(iso) + 1
        for poly in polys:
            xs = [c[0] for c in poly[0]]
            ys = [c[1] for c in poly[0]]
            shapes.append((idx, (min(xs), min(ys), max(xs), max(ys)), poly))
        if p.get("LABEL_X") is not None and iso not in labels:
            labels[iso] = (p["LABEL_Y"], p["LABEL_X"])

    def owner(lon, lat):
        for idx, (x0, y0, x1, y1), poly in shapes:
            if x0 <= lon <= x1 and y0 <= lat <= y1 and in_ring(lon, lat, poly[0]) \
                    and not any(in_ring(lon, lat, h) for h in poly[1:]):
                return idx
        return 0

    cells = bytearray(COLS * ROWS)
    sub = (0.2, 0.5, 0.8)
    for row in range(ROWS):
        for col in range(COLS):
            votes = {}
            for fy in sub:
                for fx in sub:
                    o = owner(*lonlat(col, row, fx, fy))
                    votes[o] = votes.get(o, 0) + 1
            land = {k: v for k, v in votes.items() if k}
            if sum(land.values()) >= 4:
                cells[row * COLS + col] = max(land, key=land.get)

    for iso, pos in EXTRA.items():
        if iso not in countries:
            countries.append(iso)
        labels.setdefault(iso, pos)

    anchors = {}
    for iso in countries:
        idx = countries.index(iso) + 1
        lat, lon = EXTRA.get(iso) if iso in EXTRA and iso not in labels else labels.get(iso, EXTRA.get(iso, (0, 0)))
        col, row = cell_of(lat, lon)
        own = [i for i, v in enumerate(cells) if v == idx]
        if not own:
            cells[row * COLS + col] = idx      # every country gets at least its label cell
            anchors[iso] = [col, row]
            continue
        best = min(own, key=lambda i: (i % COLS - col) ** 2 + (i // COLS - row) ** 2)
        anchors[iso] = [best % COLS, best // COLS]

    out = {"cols": COLS, "rows": ROWS, "latTop": LAT_TOP, "latBottom": LAT_BOTTOM, "countries": countries,
           "anchors": anchors, "cells": base64.b64encode(bytes(cells)).decode()}
    dst = os.path.join(HERE, "..", "src", "assets", "world.json")
    json.dump(out, open(dst, "w", encoding="utf-8"), separators=(",", ":"))
    land = sum(1 for v in cells if v)
    print(f"{COLS}x{ROWS} cells, {land} land, {len(countries)} countries -> {os.path.normpath(dst)}")
    for row in range(0, ROWS, 2):
        print("".join("#" if cells[row * COLS + c] else " " for c in range(0, COLS, 2)))


if __name__ == "__main__":
    main()

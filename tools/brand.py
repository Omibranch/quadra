"""Makes the pictures built from the cube render: the README logo and the installer art.

    python tools/brand.py <still.png from tools/intro_cube.py --still>

Writes docs/img/logo.png, src-tauri/installer/header.bmp (150x57) and sidebar.bmp (164x314).
"""
import os
import sys

from PIL import Image, ImageDraw, ImageFont

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
ACCENT = (61, 220, 132)
BG = (9, 12, 11)
INK = (228, 237, 231)
MUTED = (148, 163, 155)


def tinted(grey):
    r, g, b, a = grey.split()
    return Image.merge("RGBA", (r.point(lambda v: v * ACCENT[0] // 255), g.point(lambda v: v * ACCENT[1] // 255),
                                b.point(lambda v: v * ACCENT[2] // 255), a))


def font(size, bold=False):
    for name in (["bahnschrift.ttf"] if not bold else ["bahnschrift.ttf", "segoeuib.ttf"]) + ["segoeui.ttf", "arial.ttf"]:
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def main():
    cube = tinted(Image.open(sys.argv[1]).convert("RGBA"))
    cube = cube.crop(cube.getbbox())

    os.makedirs(os.path.join(ROOT, "docs", "img"), exist_ok=True)
    logo = cube.copy()
    logo.thumbnail((512, 512), Image.LANCZOS)
    logo.save(os.path.join(ROOT, "docs", "img", "logo.png"))

    out = os.path.join(ROOT, "src-tauri", "installer")
    os.makedirs(out, exist_ok=True)

    # the strip at the top of the installer's inner pages
    header = Image.new("RGB", (150, 57), BG)
    small = cube.copy()
    small.thumbnail((44, 44), Image.LANCZOS)
    header.paste(small, (150 - small.width - 8, (57 - small.height) // 2), small)
    header.save(os.path.join(out, "header.bmp"))

    # the tall picture on the welcome and finish pages
    side = Image.new("RGB", (164, 314), BG)
    draw = ImageDraw.Draw(side)
    for y in range(0, 314, 7):          # the map's grid of squares, fading out downwards
        for x in range(0, 164, 7):
            k = max(0.0, 1 - y / 300) * (0.10 + 0.08 * ((x * 13 + y * 7) % 5 == 0))
            c = tuple(round(BG[i] + (ACCENT[i] - BG[i]) * k) for i in range(3))
            draw.rectangle([x + 1, y + 1, x + 5, y + 5], fill=c)
    big = cube.copy()
    big.thumbnail((116, 116), Image.LANCZOS)
    side.paste(big, ((164 - big.width) // 2, 64), big)
    draw.text((82, 206), "QUADRA", font=font(20), fill=INK, anchor="mm")
    draw.text((82, 230), "VLESS client", font=font(11), fill=MUTED, anchor="mm")
    side.save(os.path.join(out, "sidebar.bmp"))
    print("logo, header.bmp, sidebar.bmp written")


if __name__ == "__main__":
    main()

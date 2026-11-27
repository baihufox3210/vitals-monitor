import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageEnhance, ImageFilter

ROOT = Path(__file__).resolve().parents[2]
ASSETS = ROOT / "assets"
ASSETS.mkdir(exist_ok=True)
SCALE = 1024

BACKGROUND_TOP = (32, 37, 46)
BACKGROUND_BOTTOM = (11, 13, 17)
ACCENT = (82, 168, 255)
MARK = (236, 247, 255)
POINTS = [
    (0.20, 0.56),
    (0.33, 0.56),
    (0.40, 0.38),
    (0.50, 0.74),
    (0.58, 0.27),
    (0.66, 0.56),
    (0.80, 0.56),
]


def rgba(hex_color: str, alpha: int = 255) -> tuple[int, int, int, int]:
    value = hex_color.lstrip("#")
    return (int(value[0:2], 16), int(value[2:4], 16), int(value[4:6], 16), alpha)


def gradient_background(width: int, height: int) -> Image.Image:
    column = Image.new("RGB", (1, height))
    for y in range(height):
        t = y / max(1, height - 1)
        column.putpixel(
            (0, y),
            tuple(round(a + (b - a) * t) for a, b in zip(BACKGROUND_TOP, BACKGROUND_BOTTOM)),
        )
    return column.resize((width, height)).convert("RGBA")


def with_glow(image: Image.Image) -> Image.Image:
    width, height = image.size
    glow = Image.new("RGBA", image.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(glow)
    draw.ellipse(
        (width * 0.26, height * 0.18, width * 0.74, height * 0.66),
        fill=(*ACCENT, 110),
    )
    glow = glow.filter(ImageFilter.GaussianBlur(width * 0.16))
    return Image.alpha_composite(image, glow)


def stroke_layer(size: int, color: tuple[int, int, int, int], thickness: int) -> Image.Image:
    layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    pen = ImageDraw.Draw(layer)
    scaled = [(round(x * size), round(y * size)) for x, y in POINTS]
    pen.line(scaled, fill=color, width=thickness, joint="curve")
    radius = thickness // 2
    for x, y in scaled:
        pen.ellipse((x - radius, y - radius, x + radius, y + radius), fill=color)
    return layer


def pulse_mark(size: int, lit: bool) -> Image.Image:
    color = (*MARK, 255) if lit else (96, 106, 122, 255)
    return stroke_layer(size, color, max(2, round(size * 0.072)))


def pulse_tile(size: int) -> Image.Image:
    background = with_glow(gradient_background(size, size)).convert("RGB")
    halo = stroke_layer(size, (*ACCENT, 255), round(size * 0.19))
    halo = halo.filter(ImageFilter.GaussianBlur(size * 0.05))
    lit_background = ImageEnhance.Brightness(background).enhance(1.45).convert("RGBA")
    tile = Image.composite(lit_background, background.convert("RGBA"), halo.getchannel("A"))
    tile = Image.alpha_composite(tile, halo)
    return Image.alpha_composite(tile, pulse_mark(size, True))


def pulse_svg() -> str:
    path = " ".join(
        f"{'M' if index == 0 else 'L'}{x * 32:.2f} {y * 32:.2f}"
        for index, (x, y) in enumerate(POINTS)
    )
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" width="32" height="32">\n'
        f'  <path d="{path}" fill="none" stroke="#ecf7ff" stroke-width="2.5" '
        'stroke-linecap="round" stroke-linejoin="round"/>\n'
        "</svg>\n"
    )


def write_icons() -> None:
    (ASSETS / "vitals.svg").write_text(pulse_svg(), encoding="utf-8")
    mark = pulse_mark(SCALE, True)
    mark.resize((256, 256), Image.LANCZOS).save(ASSETS / "vitals.png")
    tile = pulse_tile(SCALE)
    tile.resize((256, 256), Image.LANCZOS).save(ASSETS / "vitals-monitor.png")
    icon_dir = ASSETS / "icons"
    if icon_dir.exists():
        shutil.rmtree(icon_dir)
    icon_dir.mkdir()
    for size in range(16, 513, 16):
        tile.resize((size, size), Image.LANCZOS).save(icon_dir / f"vitals-monitor-{size}.png")


def write_hero() -> None:
    width, height = 1600, 720
    hero = with_glow(gradient_background(width, height))
    card = Image.new("RGBA", (width - 260, height - 220), (0, 0, 0, 0))
    pen = ImageDraw.Draw(card)
    pen.rounded_rectangle(
        (0, 0, card.width - 1, card.height - 1),
        radius=40,
        fill=(255, 255, 255, 20),
        outline=(255, 255, 255, 32),
        width=2,
    )
    hero.alpha_composite(card, (130, 110))
    hero.alpha_composite(pulse_mark(300, True), (190, 230))

    bars = [(0.10, 0.62), (0.32, 0.86), (0.54, 0.44), (0.76, 0.72)]
    baseline = 470
    for index, (x, ratio) in enumerate(bars):
        bar_height = round(300 * ratio)
        left = round(780 + x * 620)
        color = (*ACCENT, 230) if index % 2 == 0 else (255, 255, 255, 210)
        pen_bars = ImageDraw.Draw(hero)
        pen_bars.rounded_rectangle(
            (left, baseline - bar_height, left + 54, baseline), radius=27, fill=color
        )
    pen_bars = ImageDraw.Draw(hero)
    pen_bars.rounded_rectangle((780, baseline + 26, 1400, baseline + 34), radius=4, fill=(255, 255, 255, 36))
    hero.convert("RGB").save(ASSETS / "hero.png")


def write_social() -> None:
    width, height = 1280, 640
    social = with_glow(gradient_background(width, height))
    social.alpha_composite(pulse_mark(280, True), ((width - 280) // 2, (height - 280) // 2 - 20))
    social.convert("RGB").save(ASSETS / "social-preview.png")


def main() -> None:
    write_icons()
    write_hero()
    write_social()
    if shutil.which("optipng"):
        subprocess.run(
            ["optipng", "-quiet", "-o2", str(ASSETS / "hero.png"), str(ASSETS / "social-preview.png")],
            check=False,
        )


if __name__ == "__main__":
    main()

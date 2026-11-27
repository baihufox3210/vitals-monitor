import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageEnhance, ImageFilter

LANCZOS = Image.Resampling.LANCZOS

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


def write_icons() -> None:
    tile = pulse_tile(SCALE)
    tile.resize((256, 256), LANCZOS).save(ASSETS / "vitals-monitor.png")
    icon_dir = ASSETS / "icons"
    if icon_dir.exists():
        shutil.rmtree(icon_dir)
    icon_dir.mkdir()
    for size in range(16, 513, 16):
        tile.resize((size, size), LANCZOS).save(icon_dir / f"vitals-monitor-{size}.png")


def write_social() -> None:
    width, height = 1280, 640
    social = with_glow(gradient_background(width, height))
    social.alpha_composite(pulse_mark(280, True), ((width - 280) // 2, (height - 280) // 2 - 20))
    social.convert("RGB").save(ASSETS / "social-preview.png")


def main() -> None:
    write_icons()
    write_social()
    if shutil.which("optipng"):
        subprocess.run(
            ["optipng", "-quiet", "-o2", str(ASSETS / "social-preview.png")],
            check=False,
        )


if __name__ == "__main__":
    main()

"""Render PNG icons from assets/vitals-monitor.svg using rsvg-convert or cairosvg."""

import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
SOURCE = ASSETS / "vitals-monitor.svg"
ICON_DIR = ASSETS / "icons"
SIZES = (16, 24, 32, 48, 64, 128, 256, 512)


def render(source: Path, destination: Path, size: int) -> None:
    if shutil.which("rsvg-convert"):
        subprocess.run(
            ["rsvg-convert", "-w", str(size), "-h", str(size), str(source), "-o", str(destination)],
            check=True,
        )
        return
    try:
        import cairosvg
    except ImportError:
        raise SystemExit("install rsvg-convert (librsvg2-tools) or pip install cairosvg")
    cairosvg.svg2png(
        url=str(source),
        write_to=str(destination),
        output_width=size,
        output_height=size,
    )


def main() -> None:
    if not SOURCE.exists():
        raise SystemExit(f"missing {SOURCE}")
    ICON_DIR.mkdir(exist_ok=True)
    for size in SIZES:
        render(SOURCE, ICON_DIR / f"vitals-monitor-{size}.png", size)
    shutil.copyfile(ICON_DIR / "vitals-monitor-256.png", ASSETS / "vitals-monitor.png")


if __name__ == "__main__":
    main()

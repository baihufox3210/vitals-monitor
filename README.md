<p align="center">
  <img src="assets/vitals-monitor.png" width="112" height="112" alt="Vitals Monitor icon">
</p>

<h1 align="center">Vitals Monitor</h1>

<p align="center">
  Real-time game vitals for Wayland: a layer-shell overlay for frame pacing and hardware health,<br>
  with a native <a href="https://github.com/AvengeMedia/DankMaterialShell">DMS</a> dashboard to tune it.
</p>

<p align="center">
  <a href="https://github.com/baihufox3210/vitals-monitor/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/baihufox3210/vitals-monitor/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://www.rust-lang.org/"><img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white"></a>
  <a href="#requirements"><img alt="GTK 3" src="https://img.shields.io/badge/GTK-3-4A86CF?logo=gtk&logoColor=white"></a>
  <a href="#requirements"><img alt="Wayland layer-shell" src="https://img.shields.io/badge/Wayland-layer--shell-FFBC00?logo=wayland&logoColor=black"></a>
  <a href="#dashboard"><img alt="DMS 2.1+" src="https://img.shields.io/badge/DMS-2.1%2B-7B61FF"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-green.svg"></a>
</p>

Vitals takes its name from **vital signs**: FPS, frame time, GPU and CPU load are the numbers a game needs checked while it runs. The overlay draws them in the corner of your screen; the DMS tab lets you decide which ones matter.

## Features

- **Frame pacing first** — native and output FPS, 30-second frame-time graph, P50/P95/peak frame time, and frame stability, read incrementally from NGX frame-timing logs.
- **Hardware vitals** — GPU utilization, temperature, clock and power; VRAM usage and clock; CPU usage, temperature and clock; RAM and swap.
- **Game and I/O** — game process CPU/RAM/threads/uptime, NVMe read, write and temperature, plus network throughput.
- **Upscaling insight** — DLSS super-resolution and frame-generation cost with the effective resolution scale.
- **Built to stay out of the way** — samples only while the HUD is visible, cheap NVML reuse, non-blocking log tailing that survives rotation and truncation.
- **A dashboard that matches DMS** — pill navigation, minimal glass surfaces, disclosure rows with six-dot drag sorting, symmetric steppers and a focused CSS editor.

## Dashboard

DMS 2.1 or newer exposes **Vitals** as a Dash tab with five pages:

| Page | What it controls |
| --- | --- |
| **Vitals** | Which metrics appear, their order and individual fields. |
| **Placement** | Corner, columns, width and margins, plus auto-show while gaming. |
| **Sources** | Target process and the frame-timing log path. |
| **Style** | Custom GTK CSS, applied live in preview. |
| **Blocklist** | Processes that should never be treated as the game. |

Each metric row has its own switch on the right. Details start collapsed every time the tab opens; click a row to reveal its checkboxes. Drag the six-dot handle to reorder rows, or focus it and press <kbd>↑</kbd> / <kbd>↓</kbd>.

The header switch is part of the draft. **Preview** shows the draft without saving, and **Apply** writes the settings and restarts the overlay.

## Requirements

- Linux with a Wayland compositor that supports `wlr-layer-shell` (tested on niri)
- Rust 1.85+ (edition 2024)
- GTK 3 and `gtk-layer-shell` development headers
- NVIDIA GPU + driver for the NVML metrics (other metrics work without it)
- DMS 2.1+ for the dashboard tab

```bash
# Fedora
sudo dnf install gtk3-devel gtk-layer-shell-devel

# Arch
sudo pacman -S gtk3 gtk-layer-shell

# Ubuntu / Debian
sudo apt install libgtk-3-dev libgtk-layer-shell-dev
```

## Install

```bash
git clone https://github.com/baihufox3210/vitals-monitor
cd vitals-monitor

cargo build --release
install -Dm755 target/release/vitals-monitor ~/.local/bin/vitals-monitor
install -Dm644 dms/vitals-monitor-overlay.service ~/.config/systemd/user/vitals-monitor-overlay.service

mkdir -p ~/.config/dms/plugins/Vitals
cp dms/Vitals/*.qml dms/Vitals/plugin.json ~/.config/dms/plugins/Vitals/

systemctl --user daemon-reload
systemctl --user enable vitals-monitor-overlay.service

dms ipc call plugin-scan rescan vitalsMonitor
dms ipc call plugins enable vitalsMonitor
dms ipc call dash open plugin_vitalsMonitor
```

You can also run the GTK settings window directly with `cargo run --release`. Stop the overlay service first so two HUDs do not stack.

### Icons

`assets/vitals-monitor.png` and the hicolor set in `assets/icons/` can be installed for launchers and tray menus:

```bash
install -Dm644 assets/vitals-monitor.png \
  ~/.local/share/icons/hicolor/256x256/apps/vitals-monitor.png
for size in 16 24 32 48 64 128; do
  install -Dm644 "assets/icons/vitals-monitor-${size}.png" \
    ~/.local/share/icons/hicolor/${size}x${size}/apps/vitals-monitor.png
done
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor 2>/dev/null || true
```

Regenerate every asset with `python3 skills/assets/render.py` (needs Pillow).

### Upgrading from FrameHUD

Settings from the old name are copied to `~/.config/vitals-monitor/settings.json` on first launch, and the original file is left untouched. Remove the old service and plugin once you are happy with the new install:

```bash
systemctl --user disable --now framehud-overlay.service
rm -f ~/.config/systemd/user/framehud-overlay.service ~/.local/bin/framehud
rm -rf ~/.config/dms/plugins/FrameHUD
systemctl --user daemon-reload
```

## Configuration

Settings live in `$XDG_CONFIG_HOME/vitals-monitor/settings.json`, which defaults to `~/.config/vitals-monitor/settings.json`. The dashboard is the recommended way to edit them; the file is plain JSON with a `tiles` array describing order, visibility and per-field toggles.

The CLI surface is intentionally small:

```bash
vitals-monitor --read-config        # print the current settings as JSON
vitals-monitor --sample PATH        # read recent frames from an NGX log
vitals-monitor --hud                # run the overlay
vitals-monitor --control <op>       # apply / preview / stop-preview
```

## How the numbers are gathered

- **FPS** is shown as `native / output`. The left value comes from the last second of NGX frame times; the right prefers an existing GNUMON capture and otherwise estimates from generated frames in the NGX log. Missing data shows `--`.
- **Frame time** covers the last 30 seconds, including average, peak and a waveform drawn from NGX `Evaluate` intervals.
- **GPU, VRAM, CPU and RAM** use a reused NVML connection and cached CPU temperature paths, sampled once per second.
- **Game process** and **blocklist** matching ignore case and fall back gracefully when an older binary lacks a command.

## Architecture

| Module | Responsibility |
| --- | --- |
| `config` | Metric definitions, layout and settings persistence, including legacy migration. |
| `frames` | Incremental NGX log tailing, process tracking, rotation and truncation handling. |
| `gnumon` | One non-blocking CLI per target that exposes a capture ring. |
| `system` | NVML reuse and per-second system sampling. |
| `overlay` | Layer-shell surface, charts and input regions; sampling stops while hidden. |
| `settings` | GTK settings window for layout, CSS and sources. |
| `dms/Vitals` | The DMS dashboard tab (QML). |

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

`cargo run --release -- --sample /path/to/frame_timing.txt` reads a log without starting the overlay.

## License

[MIT](LICENSE)

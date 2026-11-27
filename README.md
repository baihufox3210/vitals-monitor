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
  <a href="#requirements"><img alt="DMS 2.1+" src="https://img.shields.io/badge/DMS-2.1%2B-7B61FF"></a>
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-green.svg"></a>
</p>

## Features

- Monitor FPS, frame time, GPU, CPU, memory, disk and network activity.
- Configure metrics, placement, sources and CSS in the DMS dashboard or GTK settings window.
- Preview changes before applying them; reorder metrics by dragging their handles.

Frame metrics depend on available NGX logs or GNUMON capture data. NVIDIA metrics require NVML.

## Requirements

- Linux with a Wayland compositor that supports `wlr-layer-shell` (tested on niri)
- GTK 3 and `gtk-layer-shell` (installed automatically as package dependencies on Arch)
- A current stable Rust toolchain and GTK development headers if building from source
- NVIDIA GPU + driver for the NVML metrics (other metrics work without it)
- DMS 2.1+ for the dashboard tab

## Install

### Arch Linux (x86_64)

Install the prebuilt package from the latest [GitHub Release](https://github.com/baihufox3210/vitals-monitor/releases):

```bash
curl -fL -o vitals-monitor-x86_64.pkg.tar.zst https://github.com/baihufox3210/vitals-monitor/releases/latest/download/vitals-monitor-x86_64.pkg.tar.zst
sudo pacman -U ./vitals-monitor-x86_64.pkg.tar.zst
```

Then enable the user service:

```bash
systemctl --user daemon-reload
systemctl --user enable vitals-monitor-overlay.service
```

In DMS, rescan plugins and enable **Vitals Monitor** in plugin settings. The package installs the plugin to `/etc/xdg/quickshell/dms-plugins/Vitals`, a system plugin location supported by DMS 2.1.2. A Wayland graphical session is required. The Release URL becomes available after the first Release is published. Release packages are currently unsigned; systems requiring local package signatures will reject them. Each release includes a SHA-256 checksum for detecting download corruption, but it is not a cryptographic signature.

To update, download and install the latest package again, then reload systemd and restart the service if it is running: `systemctl --user daemon-reload && systemctl --user try-restart vitals-monitor-overlay.service`. Rescan or reload the plugin in DMS. This package does not update automatically with `pacman -Syu`. Remove it with `sudo pacman -R vitals-monitor`.

If you previously installed manually, an existing `~/.config/dms/plugins/Vitals` or `~/.config/systemd/user/vitals-monitor-overlay.service` takes precedence over the packaged version. Back up and remove those copies after verifying you no longer need them; reload systemd and rescan DMS afterwards. Your settings in `~/.config/vitals-monitor/` are not removed by pacman.

To build the development version instead, clone this repository and run `makepkg -si`. The included `PKGBUILD` is not published to the AUR.

### Manual install

Install Rust and the GTK development dependencies first. For example: `sudo pacman -S gtk3 gtk-layer-shell` on Arch, `sudo dnf install gtk3-devel gtk-layer-shell-devel` on Fedora, or `sudo apt install libgtk-3-dev libgtk-layer-shell-dev` on Ubuntu / Debian.

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


### Upgrading from FrameHUD

Settings from the old name are copied to `~/.config/vitals-monitor/settings.json` on first launch, and the original file is left untouched. Remove the old service and plugin once you are happy with the new install:

```bash
systemctl --user disable --now framehud-overlay.service
rm -f ~/.config/systemd/user/framehud-overlay.service ~/.local/bin/framehud
rm -rf ~/.config/dms/plugins/FrameHUD
systemctl --user daemon-reload
```

## Configuration

Settings live in `$XDG_CONFIG_HOME/vitals-monitor/settings.json` (normally `~/.config/vitals-monitor/settings.json`). Use the DMS dashboard or GTK settings window to choose the target process, data sources and metrics. The default NGX log search targets Wuthering Waves; other games may require a custom source.

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

`cargo run --release -- --sample /path/to/frame_timing.txt` reads a log without starting the overlay.

## License

[MIT](LICENSE)

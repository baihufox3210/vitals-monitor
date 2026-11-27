# Maintainer: baihufox3210
pkgname=vitals-monitor-git
pkgver=0.1.0.r0.g0000000
pkgrel=1
pkgdesc='Wayland game performance overlay with a DMS dashboard'
arch=('x86_64')
url='https://github.com/baihufox3210/vitals-monitor'
license=('MIT')
depends=('gtk3' 'gtk-layer-shell' 'systemd')
makedepends=('git' 'rust' 'pkgconf')
optdepends=('nvidia-utils: NVIDIA GPU and VRAM metrics'
            'dms-shell: DMS dashboard integration')
provides=('vitals-monitor')
conflicts=('vitals-monitor')
options=('!debug')
install=vitals-monitor.install
source=('vitals-monitor::git+https://github.com/baihufox3210/vitals-monitor.git')
sha256sums=('SKIP')

pkgver() {
  cd "$srcdir/vitals-monitor"
  local version
  version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | sed -n '1p')
  printf '%s.r%s.g%s' "${version//-/.}" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

build() {
  cd "$srcdir/vitals-monitor"
  cargo build --release --locked
}

check() {
  cd "$srcdir/vitals-monitor"
  cargo test --locked
}

package() {
  cd "$srcdir/vitals-monitor"
  install -Dm755 target/release/vitals-monitor "$pkgdir/usr/bin/vitals-monitor"
  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/vitals-monitor/LICENSE"
  install -Dm644 assets/vitals-monitor.svg "$pkgdir/usr/share/icons/hicolor/scalable/apps/vitals-monitor.svg"

  install -Dm644 dms/vitals-monitor-overlay.service "$pkgdir/usr/lib/systemd/user/vitals-monitor-overlay.service"
  grep -Fq '%h/.local/bin/vitals-monitor' "$pkgdir/usr/lib/systemd/user/vitals-monitor-overlay.service"
  sed -i 's|%h/.local/bin/vitals-monitor|/usr/bin/vitals-monitor|' "$pkgdir/usr/lib/systemd/user/vitals-monitor-overlay.service"

  install -dm755 "$pkgdir/etc/xdg/quickshell/dms-plugins/Vitals"
  install -m644 dms/Vitals/*.qml dms/Vitals/plugin.json "$pkgdir/etc/xdg/quickshell/dms-plugins/Vitals/"
  grep -Fq 'Quickshell.env("HOME") + "/.local/bin/vitals-monitor"' "$pkgdir/etc/xdg/quickshell/dms-plugins/Vitals/HudSession.qml"
  sed -i 's|Quickshell.env("HOME") + "/.local/bin/vitals-monitor"|"/usr/bin/vitals-monitor"|; s|~/.local/bin/vitals-monitor|/usr/bin/vitals-monitor|' "$pkgdir/etc/xdg/quickshell/dms-plugins/Vitals/HudSession.qml"
}

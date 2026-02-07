# Maintainer: p4block <https://github.com/p4block>
pkgname=vibebar-p4-git
_pkgname=vibebar-p4
pkgver=0.1.0.r8.g7a2b3c4
pkgrel=1
pkgdesc="A purely vibe-coded status bar for personal use, built with Rust and GTK4."
arch=('x86_64')
url="https://github.com/p4block/vibebar-p4"
license=('WTFPL')
depends=('gtk4' 'gtk4-layer-shell' 'libpulse' 'dbus')
makedepends=('cargo' 'git' 'pkgconf')
provides=("$_pkgname")
conflicts=("$_pkgname")
source=("git+https://github.com/p4block/vibebar-p4.git")
sha256sums=('SKIP')

pkgver() {
  cd "$_pkgname"
  git describe --long --tags 2>/dev/null | sed 's/\([^-]*-g\)/r\1/;s/-/./g' || \
  printf "0.1.0.r%s.g%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

prepare() {
  cd "$_pkgname"
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd "$_pkgname"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo build --frozen --release --all-features
}

package() {
  cd "$_pkgname"
  install -Dm755 "target/release/$_pkgname" "$pkgdir/usr/bin/$_pkgname"
  # install -Dm644 README.md "$pkgdir/usr/share/doc/$_pkgname/README.md"
}

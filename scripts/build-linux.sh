#!/usr/bin/env bash
# Build Linux : paquets .deb, .rpm et AppImage.
#
#   scripts/build-linux.sh                  les trois formats
#   scripts/build-linux.sh --bundles deb    un seul format (deb, rpm, appimage)
#   scripts/build-linux.sh --fresh          réinstalle les dépendances npm avant
#
# À lancer sur Linux : Tauri ne sait pas compiler pour Linux depuis macOS ou Windows.
set -euo pipefail
cd "$(dirname "$0")/.."

BUNDLES="deb,rpm,appimage"
FRESH=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundles) BUNDLES="${2:?--bundles attend une liste, ex. deb,appimage}"; shift ;;
    --fresh) FRESH=1 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "Option inconnue : $1" >&2; exit 1 ;;
  esac
  shift
done

step() { printf '\n\033[1;35m▸ %s\033[0m\n' "$1"; }
fail() { printf '\033[1;31m✕ %s\033[0m\n' "$1" >&2; exit 1; }

[[ "$(uname -s)" == "Linux" ]] || fail "Ce script se lance sur Linux (macOS : scripts/build-macos.sh)."

step "Vérification des outils"
command -v cargo >/dev/null || fail "Rust est introuvable : https://rustup.rs"
command -v npm >/dev/null || fail "Node.js est introuvable : https://nodejs.org (version 20 ou plus)"
command -v pkg-config >/dev/null || fail "pkg-config est introuvable (voir la liste des paquets ci-dessous)."

# Bibliothèques système requises par Tauri (WebKitGTK) et par le trousseau (D-Bus).
missing=()
for lib in webkit2gtk-4.1 gtk+-3.0 libsoup-3.0 librsvg-2.0 dbus-1; do
  pkg-config --exists "$lib" || missing+=("$lib")
done
if [[ ${#missing[@]} -gt 0 ]]; then
  echo "Bibliothèques manquantes : ${missing[*]}"
  if command -v apt-get >/dev/null; then
    echo "  sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev pkg-config"
  elif command -v dnf >/dev/null; then
    echo "  sudo dnf install -y webkit2gtk4.1-devel openssl-devel curl wget file libxdo-devel libappindicator-gtk3-devel librsvg2-devel dbus-devel pkgconf-pkg-config"
    echo "  sudo dnf group install -y \"c-development\""
  elif command -v pacman >/dev/null; then
    echo "  sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg dbus xdotool"
  elif command -v zypper >/dev/null; then
    echo "  sudo zypper in -y webkit2gtk3-devel libopenssl-devel curl wget file libappindicator3-1 librsvg-devel dbus-1-devel"
  fi
  fail "Installe les paquets ci-dessus puis relance le script."
fi
echo "cargo $(cargo --version | cut -d' ' -f2) · node $(node --version) · $(uname -m)"

if [[ $FRESH == 1 || ! -d node_modules ]]; then
  step "Installation des dépendances npm"
  npm ci
fi

step "Compilation (quelques minutes la première fois)"
npm run tauri build -- --bundles "$BUNDLES"

step "Terminé"
find src-tauri/target/release/bundle -maxdepth 2 -type f \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) \
  -exec du -h {} \; | sed 's/^/  /'

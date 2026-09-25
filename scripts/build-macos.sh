#!/usr/bin/env bash
# Build macOS : Magisterium.app + DMG.
#
#   scripts/build-macos.sh              architecture de cette machine
#   scripts/build-macos.sh --universal  binaire universel (Apple Silicon + Intel)
#   scripts/build-macos.sh --fresh      réinstalle les dépendances npm avant
set -euo pipefail
cd "$(dirname "$0")/.."

UNIVERSAL=0
FRESH=0
for arg in "$@"; do
  case "$arg" in
    --universal) UNIVERSAL=1 ;;
    --fresh) FRESH=1 ;;
    -h|--help) sed -n '2,7p' "$0"; exit 0 ;;
    *) echo "Option inconnue : $arg" >&2; exit 1 ;;
  esac
done

step() { printf '\n\033[1;35m▸ %s\033[0m\n' "$1"; }
fail() { printf '\033[1;31m✕ %s\033[0m\n' "$1" >&2; exit 1; }

[[ "$(uname -s)" == "Darwin" ]] || fail "Ce script se lance sur macOS (Linux : scripts/build-linux.sh)."

step "Vérification des outils"
command -v cargo >/dev/null || fail "Rust est introuvable : https://rustup.rs"
command -v npm >/dev/null || fail "Node.js est introuvable : https://nodejs.org (version 20 ou plus)"
xcode-select -p >/dev/null 2>&1 || fail "Outils de ligne de commande Xcode absents : xcode-select --install"
echo "cargo $(cargo --version | cut -d' ' -f2) · node $(node --version) · $(uname -m)"

if [[ $FRESH == 1 || ! -d node_modules ]]; then
  step "Installation des dépendances npm"
  npm ci
fi

TARGET_ARGS=()
BUNDLE_DIR="src-tauri/target/release/bundle"
if [[ $UNIVERSAL == 1 ]]; then
  step "Cibles Rust pour le binaire universel"
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  TARGET_ARGS=(--target universal-apple-darwin)
  BUNDLE_DIR="src-tauri/target/universal-apple-darwin/release/bundle"
fi

step "Compilation (quelques minutes la première fois)"
npm run tauri build -- --bundles app,dmg ${TARGET_ARGS[@]+"${TARGET_ARGS[@]}"}

step "Terminé"
find "$BUNDLE_DIR/dmg" -name '*.dmg' -maxdepth 1 -print0 | while IFS= read -r -d '' f; do
  hdiutil verify "$f" >/dev/null 2>&1 && ok="vérifié" || ok="NON vérifié"
  printf '  %s  (%s, %s)\n' "$f" "$(du -h "$f" | cut -f1 | tr -d ' ')" "$ok"
done
echo "  $BUNDLE_DIR/macos/Magisterium.app"

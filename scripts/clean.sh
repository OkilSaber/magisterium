#!/usr/bin/env bash
# Nettoie les artefacts de build (macOS et Linux).
#
#   scripts/clean.sh              dist/, src-tauri/target/, src-tauri/gen/
#   scripts/clean.sh --bundles    seulement les paquets produits (DMG, .app, .deb…)
#   scripts/clean.sh --all        tout, y compris node_modules/
#   scripts/clean.sh --dry-run    affiche ce qui serait supprimé, sans rien toucher
#
# Ne touche jamais aux données de l'app (discussions, fournisseurs, clés API).
set -euo pipefail
cd "$(dirname "$0")/.."

MODE="build"
DRY=0
for arg in "$@"; do
  case "$arg" in
    --bundles) MODE="bundles" ;;
    --all) MODE="all" ;;
    --dry-run|-n) DRY=1 ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "Option inconnue : $arg" >&2; exit 1 ;;
  esac
done

case "$MODE" in
  bundles) targets=(src-tauri/target/release/bundle src-tauri/target/*/release/bundle) ;;
  build) targets=(dist src-tauri/target src-tauri/gen) ;;
  all) targets=(dist src-tauri/target src-tauri/gen node_modules) ;;
esac

total=0
found=0
for t in "${targets[@]}"; do
  [[ -e "$t" ]] || continue
  found=1
  kb=$(du -sk "$t" | cut -f1)
  total=$((total + kb))
  printf '  %-45s %s\n' "$t" "$(du -sh "$t" | cut -f1 | tr -d ' ')"
  [[ $DRY == 1 ]] || rm -rf "$t"
done

if [[ $found == 0 ]]; then
  echo "Rien à nettoyer."
elif [[ $DRY == 1 ]]; then
  echo "Simulation : $((total / 1024)) Mo seraient libérés (relance sans --dry-run pour supprimer)."
else
  echo "Libéré : $((total / 1024)) Mo."
fi

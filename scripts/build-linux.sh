#!/usr/bin/env bash
# Linux 向け配布物（.AppImage / .deb）をビルドして release/ に集める。
# Usage: bash scripts/build-linux.sh [version]
set -euo pipefail
cd "$(dirname "$0")/.."

missing=()
for pkg in webkit2gtk-4.1 gtk+-3.0 librsvg-2.0; do
  pkg-config --exists "$pkg" 2>/dev/null || missing+=("$pkg")
done
for cmd in file; do
  command -v "$cmd" >/dev/null || missing+=("$cmd")
done
if ((${#missing[@]})); then
  echo "❌ 不足している依存: ${missing[*]}"
  echo "   sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \\"
  echo "     libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev"
  exit 1
fi

[ -d node_modules ] || npm ci
[ -n "${1:-}" ] && node scripts/set-version.mjs "$1"
node scripts/generate-third-party-licenses.mjs

npm run tauri build -- --bundles appimage,deb
node scripts/collect-artifacts.mjs --label linux-x86_64

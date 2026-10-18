#!/usr/bin/env bash
# 【実験的】Linux から Windows 版（NSIS インストーラー + ポータブルzip）をクロスビルドする。
# MSI は WiX が Windows 専用のため作成できない。正式な配布物は GitHub Actions で作ること。
# Usage: bash scripts/build-windows-cross.sh [version]
set -euo pipefail
cd "$(dirname "$0")/.."

need() {
  command -v "$1" >/dev/null || { echo "❌ $1 が見つかりません → $2"; exit 1; }
}
need makensis   "sudo apt-get install -y nsis"
need clang      "sudo apt-get install -y clang lld llvm"
need lld-link   "sudo apt-get install -y lld"
need llvm-rc    "sudo apt-get install -y llvm（llvm-rc-XX しか無い場合は llvm-rc という名前でリンクを作成）"
need cargo-xwin "cargo install --locked cargo-xwin"

rustup target add x86_64-pc-windows-msvc
[ -d node_modules ] || npm ci
[ -n "${1:-}" ] && node scripts/set-version.mjs "$1"
node scripts/generate-third-party-licenses.mjs

npm run tauri build -- --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
node scripts/collect-artifacts.mjs --target x86_64-pc-windows-msvc --label windows-x64

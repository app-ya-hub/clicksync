#!/usr/bin/env bash
# macOS 向け配布物（Universal Binary: Apple Silicon + Intel の .dmg / .app）をビルドする。
# macOS 上でのみ実行可能（Linux からは GitHub Actions を使う）。
# Usage: bash scripts/build-macos.sh [version]
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "$(uname)" != "Darwin" ]]; then
  echo "❌ macOS 版は macOS 上でのみビルドできます。GitHub Actions の Release ワークフローを使ってください。"
  exit 1
fi

rustup target add aarch64-apple-darwin x86_64-apple-darwin
[ -d node_modules ] || npm ci
[ -n "${1:-}" ] && node scripts/set-version.mjs "$1"
node scripts/generate-third-party-licenses.mjs

npm run tauri build -- --target universal-apple-darwin --bundles app,dmg
node scripts/collect-artifacts.mjs --target universal-apple-darwin --label macos-universal

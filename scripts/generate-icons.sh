#!/usr/bin/env bash
# アプリアイコン一式（.ico / .icns / PNG）を1枚の元画像から生成する。
# 元画像: src-tauri/app-icon.svg（または 1024×1024 の透過PNGを引数で指定）
# Usage: bash scripts/generate-icons.sh [source]
set -euo pipefail
cd "$(dirname "$0")/.."

SOURCE="${1:-src-tauri/app-icon.svg}"
[ -f "$SOURCE" ] || { echo "❌ $SOURCE が見つかりません"; exit 1; }

npx tauri icon "$SOURCE" -o src-tauri/icons
# モバイル用は使わない
rm -rf src-tauri/icons/android src-tauri/icons/ios
echo "✅ アイコンを生成しました → src-tauri/icons/"

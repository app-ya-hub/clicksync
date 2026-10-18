#!/usr/bin/env node
// `tauri build` の成果物を release/ に配布用のファイル名で集める（Windows は ポータブルzip も作成）。
// Usage: node scripts/collect-artifacts.mjs --label <linux-x86_64|windows-x64|macos-universal> [--target <rust target triple>]
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const arg = (name) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : undefined;
};
const target = arg('target');
const label = arg('label') ?? `${process.platform}-${process.arch}`;

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { productName, version } = JSON.parse(readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
// cargo は src-tauri で実行されるため、相対の CARGO_TARGET_DIR は src-tauri 基準
const targetRoot = process.env.CARGO_TARGET_DIR
  ? resolve(root, 'src-tauri', process.env.CARGO_TARGET_DIR)
  : join(root, 'src-tauri/target');
const releaseDir = join(targetRoot, ...(target ? [target] : []), 'release');
const bundleDir = join(releaseDir, 'bundle');
const outDir = join(root, 'release');
const base = `${productName}_${version}_${label}`;
mkdirSync(outDir, { recursive: true });

const collected = [];

/** bundle/<sub> から今回のバージョンのファイルを1つ選ぶ（古いビルドの残骸を拾わない） */
function pick(sub, ext) {
  const dir = join(bundleDir, sub);
  if (!existsSync(dir)) return null;
  const files = readdirSync(dir).filter((n) => n.endsWith(ext) && n.includes(version));
  return files.length ? join(dir, files.sort().at(-1)) : null;
}

for (const [sub, ext, suffix] of [
  ['nsis', '.exe', '-setup.exe'],
  ['msi', '.msi', '.msi'],
  ['dmg', '.dmg', '.dmg'],
  ['appimage', '.AppImage', '.AppImage'],
  ['deb', '.deb', '.deb'],
  ['rpm', '.rpm', '.rpm'],
]) {
  const src = pick(sub, ext);
  if (!src) continue;
  const dst = join(outDir, base + suffix);
  cpSync(src, dst);
  collected.push(dst);
}

// macOS: .app を zip でも配布（dmg を開けない環境向け）
const app = join(bundleDir, 'macos', `${productName}.app`);
if (process.platform === 'darwin' && existsSync(app)) {
  const dst = join(outDir, `${base}.app.zip`);
  rmSync(dst, { force: true });
  execFileSync('ditto', ['-c', '-k', '--keepParent', app, dst]);
  collected.push(dst);
}

// Windows: インストール不要のポータブル版
const exe = join(releaseDir, 'clicksync.exe');
if (existsSync(exe)) {
  const staging = join(outDir, '.portable');
  const folder = join(staging, productName);
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(folder, { recursive: true });
  cpSync(exe, join(folder, `${productName}.exe`));
  writeFileSync(join(folder, 'はじめにお読みください.txt'), PORTABLE_README().replace(/\n/g, '\r\n'));
  for (const [src, name] of [
    [join(root, 'LICENSE'), 'LICENSE.txt'],
    [join(root, 'src-tauri/resources/THIRD_PARTY_LICENSES.txt'), 'THIRD_PARTY_LICENSES.txt'],
  ]) {
    if (existsSync(src)) cpSync(src, join(folder, name));
  }

  const dst = join(outDir, `${base}-portable.zip`);
  rmSync(dst, { force: true });
  if (process.platform === 'win32') {
    execFileSync('tar', ['-a', '-c', '-f', dst, '-C', staging, productName]);
  } else {
    execFileSync('zip', ['-r', '-q', dst, productName], { cwd: staging });
  }
  rmSync(staging, { recursive: true, force: true });
  collected.push(dst);
}

if (collected.length === 0) {
  console.error(`❌ 成果物が見つかりません: ${bundleDir}`);
  process.exit(1);
}
console.log('配布ファイル:');
for (const f of collected) {
  console.log(`  ${f.slice(root.length + 1)}  (${(statSync(f).size / 1024 / 1024).toFixed(1)} MB)`);
}

function PORTABLE_README() {
  return `${productName} ${version} ポータブル版

■ 起動方法
${productName}.exe をダブルクリックしてください。インストールは不要です。

■ 動作環境
Windows 10 / 11 (64bit)
画面表示に Microsoft Edge WebView2 ランタイムを使います（Windows 10/11 には通常インストール済み）。
起動しない場合は https://developer.microsoft.com/microsoft-edge/webview2/ からインストールするか、
インストーラー版（*-setup.exe）をご利用ください。

■ 設定の保存場所
APIキーなどの設定は %APPDATA%\\com.clicksync.desktop に保存されます。
`;
}

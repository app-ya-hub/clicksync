#!/usr/bin/env node
// アプリのバージョンを tauri.conf.json / Cargo.toml / package.json にまとめて反映する。
// Usage: node scripts/set-version.mjs 1.2.3   （先頭の v は省略可）
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const version = (process.argv[2] ?? '').replace(/^v/, '');
// MSI (WiX) は「数字.数字.数字」以外のバージョンを受け付けない
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('Usage: node scripts/set-version.mjs <major.minor.patch>   例: 1.0.0');
  process.exit(1);
}

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

function patchJson(rel, apply) {
  const path = join(root, rel);
  if (!existsSync(path)) return;
  const json = JSON.parse(readFileSync(path, 'utf8'));
  apply(json);
  writeFileSync(path, JSON.stringify(json, null, 2) + '\n');
  console.log(`  ✓ ${rel}`);
}

patchJson('src-tauri/tauri.conf.json', (j) => (j.version = version));
patchJson('package.json', (j) => (j.version = version));
patchJson('package-lock.json', (j) => {
  j.version = version;
  if (j.packages?.['']) j.packages[''].version = version;
});

const cargoPath = join(root, 'src-tauri/Cargo.toml');
const cargo = readFileSync(cargoPath, 'utf8');
writeFileSync(cargoPath, cargo.replace(/^version = ".*"$/m, `version = "${version}"`));
console.log('  ✓ src-tauri/Cargo.toml');

console.log(`バージョンを ${version} に設定しました`);

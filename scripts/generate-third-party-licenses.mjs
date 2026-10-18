#!/usr/bin/env node
// 配布物に含まれるオープンソースのライセンス表記を src-tauri/resources/THIRD_PARTY_LICENSES.txt に生成する。
//   - Rust クレート: cargo metadata（全OS分）
//   - フロントエンド: npm の本番依存（--omit=dev）
// 同じ本文のライセンスは1回だけ載せ、使用しているパッケージを列挙する。
// Usage: node scripts/generate-third-party-licenses.mjs
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const outPath = join(root, 'src-tauri/resources/THIRD_PARTY_LICENSES.txt');
const LICENSE_FILE = /^(licen[cs]e|copying|notice|unlicense)([-_.].*)?$/i;

/** @type {{name: string, version: string, license: string, repository: string, dir: string}[]} */
const packages = [];

// ── Rust ──────────────────────────────────────────────────────────────────────
const metadata = JSON.parse(
  execFileSync(
    'cargo',
    ['metadata', '--format-version', '1', '--manifest-path', join(root, 'src-tauri/Cargo.toml')],
    { maxBuffer: 256 * 1024 * 1024 },
  ).toString(),
);
const workspace = new Set(metadata.workspace_members);
for (const p of metadata.packages) {
  if (workspace.has(p.id)) continue;
  packages.push({
    name: p.name,
    version: p.version,
    license: p.license ?? (p.license_file ? `ファイル: ${p.license_file}` : '不明'),
    repository: p.repository ?? '',
    dir: dirname(p.manifest_path),
  });
}

// ── npm（本番依存のみ） ───────────────────────────────────────────────────────
let tree = {};
try {
  tree = JSON.parse(
    execFileSync('npm', ['ls', '--omit=dev', '--all', '--json'], { cwd: root, maxBuffer: 64 * 1024 * 1024 }).toString(),
  );
} catch (e) {
  // npm ls は未解決の optional 依存があると非0で終わるが、JSON は出力される
  tree = JSON.parse(e.stdout?.toString() || '{}');
}
const seenNpm = new Set();
(function walk(deps = {}) {
  for (const [name, info] of Object.entries(deps)) {
    const key = `${name}@${info.version}`;
    if (seenNpm.has(key)) continue;
    seenNpm.add(key);
    const dir = join(root, 'node_modules', name);
    const pkgJson = join(dir, 'package.json');
    if (existsSync(pkgJson)) {
      const pkg = JSON.parse(readFileSync(pkgJson, 'utf8'));
      const repo = typeof pkg.repository === 'string' ? pkg.repository : pkg.repository?.url ?? '';
      packages.push({
        name,
        version: pkg.version,
        license: typeof pkg.license === 'string' ? pkg.license : pkg.license?.type ?? '不明',
        repository: repo.replace(/^git\+/, ''),
        dir,
      });
    }
    walk(info.dependencies);
  }
})(tree.dependencies);

// ── 本文の収集と重複排除 ───────────────────────────────────────────────────────
/** @type {Map<string, {text: string, users: string[]}>} */
const texts = new Map();
const withoutText = [];
for (const p of packages.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version))) {
  const label = `${p.name} ${p.version} (${p.license})${p.repository ? ` ${p.repository}` : ''}`;
  const files = existsSync(p.dir) ? readdirSync(p.dir).filter((f) => LICENSE_FILE.test(f)).sort() : [];
  if (files.length === 0) {
    withoutText.push(label);
    continue;
  }
  for (const f of files) {
    const text = readFileSync(join(p.dir, f), 'utf8').replace(/\r\n/g, '\n').trim();
    if (!text) continue;
    const hash = createHash('sha256').update(text.replace(/\s+/g, ' ')).digest('hex');
    if (!texts.has(hash)) texts.set(hash, { text, users: [] });
    const entry = texts.get(hash);
    if (!entry.users.includes(label)) entry.users.push(label);
  }
}

// ── 出力 ──────────────────────────────────────────────────────────────────────
const HR = '='.repeat(80);
const out = [
  'ClickSync で使用しているオープンソースソフトウェア',
  'Third-party software notices for ClickSync',
  '',
  '本ソフトウェアには、以下のオープンソースソフトウェアが含まれています。',
  '各ソフトウェアの著作権はそれぞれの著作者に帰属し、各ライセンスに従って使用しています。',
  '',
  `パッケージ数: ${packages.length}`,
  '',
  HR,
  'Linux 版（AppImage）に同梱されるシステムライブラリ',
  HR,
  'Linux 版 AppImage には、WebKitGTK・GTK・GLib などのシステムライブラリが同梱されます。',
  'これらは主に GNU LGPL 2.1 以降のもとで配布されており、動的リンクで使用しています。',
  'AppImage は `--appimage-extract` で展開でき、利用者はこれらのライブラリを差し替えられます。',
  'ソースコードは各プロジェクト（https://webkitgtk.org/ 、https://www.gtk.org/ など）または',
  'Ubuntu のソースパッケージ（https://launchpad.net/ubuntu）から入手できます。',
  '',
];
const mpl = packages.filter((p) => p.license.includes('MPL'));
if (mpl.length) {
  out.push(
    HR,
    'Mozilla Public License 2.0 のコンポーネント',
    HR,
    '以下は MPL-2.0 のもとで改変せずに使用しています。ソースコードは記載のリポジトリまたは https://crates.io から入手できます。',
    ...mpl.map((p) => `${p.name} ${p.version} ${p.repository}`),
    '',
  );
}
for (const { text, users } of texts.values()) {
  out.push(HR, ...users, HR, '', text, '');
}
if (withoutText.length) {
  out.push(HR, '以下はライセンス本文がパッケージに含まれないため、記載のライセンス（SPDX 識別子）を参照してください', HR, ...withoutText, '');
}

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, out.join('\n'));
console.log(
  `✓ ${outPath.slice(root.length + 1)}（${packages.length} パッケージ / 本文 ${texts.size} 種 / ${(Buffer.byteLength(out.join('\n')) / 1024).toFixed(0)} KB）`,
);

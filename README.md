# ClickSync（クリック・シンク）

> YouTube投稿効率化コックピット — 動画の音声・字幕・台本から、サムネイル用の画像生成プロンプトとSEOメタデータを一括生成するネイティブデスクトップアプリ

---

## 特徴

| 機能 | 内容 |
|---|---|
| **ローカル素材から生成** | 音声ファイル（Geminiで文字起こし）・字幕ファイル（SRT/VTT/SBV）・台本テキストを読み込み。YouTubeには一切アクセスしません |
| **トーン自動判定** | 文字起こしから動画のトーン（感情系・ビジネス系・解説系など）を判定 |
| **サムネイル用プロンプト×3** | 見本画像の構図・配色・文字配置を踏襲した、Web版Gemini向けプロンプトを3パターン |
| **SEOメタデータ** | タイトル（＋別案）・概要欄・タグ・ハッシュタグ3〜5個・タイムスタンプ付きチャプター |
| **ワンクリックコピー** | 各プロンプト・各SEO項目をそのままコピー |
| **BYOK** | Gemini APIキーはローカル保存、開発者側サーバーなし |

画像そのものはアプリでは生成しません。Web版Geminiに見本画像を添付し、コピーしたプロンプトを貼り付けて生成してください。

動画ファイルは受け付けません（音声だけを書き出して読み込んでください）。台本テキストには時刻情報が無いため、チャプターは作成されません。
音声は文字起こしのため利用者自身の Gemini API キーで Google に送信され、処理後に削除を要求します（Google 側で最大48時間保存される場合があります）。

---

## 必要環境（開発・ビルド）

| ツール | バージョン | 用途 |
|---|---|---|
| [Rust](https://rustup.rs/) | stable (≥ 1.77) | バックエンド |
| [Node.js](https://nodejs.org/) | ≥ 20 LTS | フロントエンドビルド |
| [Tauri CLI](https://tauri.app/) | v2（npm で自動導入） | アプリバンドル |

### Linux追加依存

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

---

## 初期セットアップ（ローカル開発）

```bash
# 1. リポジトリをクローン
git clone https://github.com/YOUR_ORG/clicksync.git
cd clicksync

# 2. npm依存をインストール
npm install

# 3. 開発サーバー起動
npm run tauri dev
```

### 起動後の設定

1. アプリが起動したら **⚙ 設定** をクリック
2. [Google AI Studio](https://aistudio.google.com/app/apikey) で Gemini API キーを取得
3. 設定画面にAPIキーを入力し、必要なら見本画像（デザイン見本のサムネイル）を設定して保存
4. 音声ファイル・字幕ファイル・台本のいずれかを読み込み、必要ならテーマ・仮タイトルを入れて 🚀 生成

---

## 配布用パッケージのビルド

配布物はすべて `release/` に `ClickSync_<バージョン>_<OS>.<拡張子>` の名前で出力されます。

| OS | 配布物 | 作れる場所 |
|---|---|---|
| Windows 10/11 x64 | `-setup.exe`（NSIS）/ `.msi` / `-portable.zip` | GitHub Actions（Linux からは NSIS+zip のみ実験的に可） |
| macOS 10.15+（Apple Silicon + Intel） | `.dmg` / `.app.zip`（Universal Binary） | GitHub Actions または Mac |
| Linux x86_64 | `.AppImage`（依存ライブラリ同梱）/ `.deb` | この Linux 環境 / GitHub Actions |

### ローカル（Linux）

```bash
npm run dist:linux              # .AppImage と .deb
npm run dist:linux -- 1.0.0     # バージョンを 1.0.0 にしてからビルド
npm run dist:win-cross          # 【実験的】Windows の NSIS インストーラー + ポータブルzip
```

Mac 上では `npm run dist:mac` で Universal Binary の `.dmg` / `.app.zip` を作れます。

### GitHub Actions（全OSを自動ビルド・推奨）

```bash
git tag v1.0.0
git push origin v1.0.0
```

`.github/workflows/release.yml` が Windows / macOS / Linux を並列ビルドし、`SHA256SUMS.txt` 付きで GitHub Releases に公開します。
**Actions → Release → Run workflow** から手動実行した場合は下書きリリースになります。

- バージョンは `数字.数字.数字` のみ（MSI の制約）。タグから自動で `tauri.conf.json` / `Cargo.toml` / `package.json` に反映されます。
- アイコンを変えるときは `src-tauri/app-icon.svg`（または 1024×1024 PNG）を差し替えて `bash scripts/generate-icons.sh`。

### エンドユーザー側の前提

- **Windows**: インストーラーは WebView2 ランタイムが無ければ自動で導入します。ポータブル版は Windows 10/11 標準の WebView2 を使います。
- **macOS**: Apple 証明書なしのビルドは ad-hoc 署名のため、初回のみ右クリック →「開く」が必要です。
- **Linux**: AppImage は `chmod +x` してダブルクリックで起動できます。`.deb` は依存パッケージがインストール時に自動で入ります。

---

## Secrets 設定（GitHub Repository → Settings → Secrets）

すべて任意です。未設定でもビルド・リリースは成功します（`GITHUB_TOKEN` は自動提供）。

### macOS Apple署名・公証（Gatekeeper の警告をなくす場合）

| Secret名 | 内容 |
|---|---|
| `APPLE_CERTIFICATE` | Developer ID証明書（base64） |
| `APPLE_CERTIFICATE_PASSWORD` | 証明書パスワード |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: ...` |
| `APPLE_ID` | Apple ID メールアドレス |
| `APPLE_PASSWORD` | App-specific password |
| `APPLE_TEAM_ID` | チームID |

> Windows の SmartScreen 警告をなくすには、別途コード署名証明書（OV/EV）が必要です。

---

## プロジェクト構成

```
clicksync/
├── .github/
│   └── workflows/
│       ├── release.yml     # リリースビルド（Win/Mac）
│       └── ci.yml          # PR チェック
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── types.rs        # 全データ型定義
│   │   ├── commands/
│   │   │   ├── source.rs       # 素材の読み込み（字幕パース・台本・文字コード判定）
│   │   │   ├── transcribe.rs   # 音声を Gemini Files API で文字起こし
│   │   │   ├── gemini.rs       # Gemini API クライアント
│   │   │   ├── generate.rs     # トーン判定・サムネイル用プロンプト・SEO生成
│   │   │   ├── reference.rs    # 見本画像の構造解析（構図・配色・文字配置）
│   │   │   └── mod.rs          # IPC コマンド登録
│   │   └── store/
│   │       └── settings.rs     # APIキー暗号化保存
│   ├── assets/
│   │   └── icon-source.png          ← 任意（アイコン生成元）
│   ├── icons/               # generate-icons.sh で生成
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   └── entitlements.plist   # macOS 権限設定
├── src/
│   ├── components/
│   │   ├── layout/          # Header, Sidebar
│   │   ├── input/           # SourceInput（音声・字幕・台本）
│   │   ├── cockpit/         # CockpitWorkspace, PromptCard, SeoPanel
│   │   ├── actions/         # RegenerateButton
│   │   ├── common/          # CopyButton
│   │   └── settings/        # SettingsModal
│   ├── hooks/               # useGenerate, useSettings, useGeminiCatalog
│   ├── stores/              # appStore (Zustand)
│   ├── types/               # TypeScript型定義
│   └── utils/               # seo
├── scripts/
│   ├── build-linux.sh          # .AppImage / .deb
│   ├── build-macos.sh          # Universal .dmg / .app（Mac 上で実行）
│   ├── build-windows-cross.sh  # 【実験的】Linux → Windows NSIS / ポータブルzip
│   ├── collect-artifacts.mjs   # 成果物を release/ に配布名で集める
│   ├── set-version.mjs         # バージョンを一括反映
│   ├── generate-third-party-licenses.mjs  # OSS ライセンス表記を生成
│   └── generate-icons.sh       # app-icon.svg → .ico / .icns / PNG
├── index.html
├── package.json
├── vite.config.ts
├── tailwind.config.ts
├── tsconfig.json
├── postcss.config.js
└── README.md
```

---

## ライセンス

プロプライエタリ（All Rights Reserved）。無断での複製・再配布・販売を禁じます。詳細は [LICENSE](LICENSE) を参照してください。
利用者向けの利用規約の雛形は [docs/legal/EULA.md](docs/legal/EULA.md) にあります。

同梱しているオープンソースのライセンス表記は `npm run licenses` で
`src-tauri/resources/THIRD_PARTY_LICENSES.txt` に生成され、アプリの「設定 → オープンソースライセンス」から表示できます。
依存関係を追加・更新したら再生成してください（配布用ビルドでは自動で再生成されます）。

import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { PickedFile, SourceInput as Source, SourceKind } from '../../types';
import { useAppStore } from '../../stores/appStore';
import { useGenerate } from '../../hooks/useGenerate';
import { useSettings } from '../../hooks/useSettings';
import { useGeminiCatalog } from '../../hooks/useGeminiCatalog';

const MIN_SCRIPT_CHARS = 20;

const TABS: { kind: SourceKind; label: string; hint: string }[] = [
  { kind: 'audio', label: '🎙 音声ファイル', hint: 'MP3 / WAV / M4A / AAC / OGG / FLAC / AIFF' },
  { kind: 'subtitle', label: '💬 字幕ファイル', hint: 'SRT / VTT / SBV（タイムスタンプ付き）' },
  { kind: 'text', label: '📝 台本・テキスト', hint: '貼り付け、または TXT / MD を読み込み' },
];

export function SourceInput() {
  const { status, error, settings } = useAppStore();
  const { hasApiKey, setSettingsOpen } = useSettings();
  const { generate } = useGenerate();
  const { catalogStatus, catalogError, activeModelId, usedFallback } = useGeminiCatalog();

  const [kind, setKind] = useState<SourceKind>('audio');
  const [files, setFiles] = useState<Partial<Record<'audio' | 'subtitle', PickedFile>>>({});
  const [script, setScript] = useState('');
  const [topic, setTopic] = useState('');
  const [pickError, setPickError] = useState<string | null>(null);

  const isRunning = status === 'fetching' || status === 'generating';
  const modelsReady = catalogStatus === 'ready' || catalogStatus === 'error';

  const source: Source | null =
    kind === 'text'
      ? script.trim().length >= MIN_SCRIPT_CHARS
        ? { kind: 'text', text: script }
        : null
      : files[kind]
        ? { kind, path: files[kind]!.path }
        : null;
  const canRun = !!source && hasApiKey && !isRunning && catalogStatus !== 'loading';

  const pick = async (target: SourceKind) => {
    setPickError(null);
    try {
      const picked = await invoke<PickedFile | null>('cmd_pick_source_file', { kind: target });
      if (!picked) return;
      if (target === 'text') {
        setScript(await invoke<string>('cmd_read_text_file', { path: picked.path }));
      } else {
        setFiles((prev) => ({ ...prev, [target]: picked }));
      }
    } catch (err) {
      setPickError(typeof err === 'string' ? err : 'ファイルを開けませんでした');
    }
  };

  const run = () => {
    if (canRun && source) void generate(source, topic);
  };

  const busyLabel =
    status === 'fetching' ? (kind === 'audio' ? '文字起こし中' : '読み込み中') : '生成中';

  return (
    <div className="flex flex-col items-center justify-center flex-1 gap-5 px-8 py-6 overflow-y-auto">
      {!hasApiKey && (
        <button
          onClick={() => setSettingsOpen(true)}
          className="flex items-center gap-2 px-4 py-2 rounded-xl bg-yellow-400/10 border border-yellow-400/20 text-yellow-300 text-[12px] hover:bg-yellow-400/15 transition-colors"
        >
          <span>⚠</span>
          <span>Gemini API キーを設定してください</span>
          <span className="text-yellow-400 font-semibold">→ 設定を開く</span>
        </button>
      )}

      <div className="w-full max-w-2xl flex flex-col gap-3">
        <div className="flex gap-1 p-1 rounded-xl bg-white/5 border border-white/8">
          {TABS.map((t) => (
            <button
              key={t.kind}
              type="button"
              onClick={() => setKind(t.kind)}
              disabled={isRunning}
              className={`flex-1 px-3 py-2 rounded-lg text-[12px] font-semibold transition-colors disabled:cursor-not-allowed ${
                kind === t.kind ? 'bg-accent text-white' : 'text-white/50 hover:text-white/80 hover:bg-white/5'
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>

        {kind === 'text' ? (
          <div className="flex flex-col gap-2">
            <textarea
              value={script}
              onChange={(e) => setScript(e.target.value)}
              disabled={isRunning}
              rows={9}
              placeholder="動画の台本・話す内容・メモを貼り付けてください"
              className="w-full resize-y bg-surface-elevated border border-white/10 rounded-xl px-4 py-3 text-[13px] leading-relaxed text-white placeholder-white/25 focus:outline-none focus:border-accent/50 select-text"
            />
            <div className="flex items-center justify-between text-[11px] text-white/35">
              <button
                type="button"
                onClick={() => void pick('text')}
                disabled={isRunning}
                className="px-3 py-1.5 rounded-lg bg-white/8 hover:bg-white/14 text-white/70 disabled:opacity-35"
              >
                ファイルから読み込み…
              </button>
              <span>
                {script.trim().length.toLocaleString()} 文字
                {script.trim().length > 0 && script.trim().length < MIN_SCRIPT_CHARS && `（${MIN_SCRIPT_CHARS}文字以上必要）`}
              </span>
            </div>
            <p className="text-[11px] text-white/30">時刻情報がないため、チャプターは作成されません。</p>
          </div>
        ) : (
          <FileSlot
            file={files[kind]}
            hint={TABS.find((t) => t.kind === kind)!.hint}
            disabled={isRunning}
            onPick={() => void pick(kind)}
            onClear={() => setFiles((prev) => ({ ...prev, [kind]: undefined }))}
          />
        )}

        {kind === 'audio' && (
          <div className="rounded-xl border border-white/8 bg-white/[0.03] px-4 py-3 text-[11px] leading-relaxed text-white/45">
            <p className="font-semibold text-white/60 mb-1">音声の取り扱いについて</p>
            <ul className="list-disc pl-4 space-y-0.5">
              <li>文字起こしのため、音声はあなたの Gemini API キーで Google に送信されます。</li>
              <li>処理後すぐに削除を要求しますが、Google 側で最大48時間保存される場合があります。</li>
              <li>無料枠の API キーでは、入力内容が Google のサービス改善に使われることがあります。</li>
              <li>長い音声は文字起こしに数分かかり、API の利用料金も音声の長さに応じて増えます。</li>
            </ul>
          </div>
        )}

        <div className="flex gap-3">
          <input
            type="text"
            value={topic}
            onChange={(e) => setTopic(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && run()}
            disabled={isRunning}
            placeholder="テーマ・仮タイトル（任意）"
            className="flex-1 bg-surface-elevated border border-white/10 rounded-xl px-4 py-3 text-[13px] text-white placeholder-white/25 focus:outline-none focus:border-accent/50 transition-all"
          />
          <button
            onClick={run}
            disabled={!canRun}
            className="px-6 py-3 bg-accent hover:bg-accent-hover disabled:opacity-35 disabled:cursor-not-allowed rounded-xl text-[13px] font-bold transition-all shrink-0"
          >
            {isRunning ? (
              <span className="flex items-center gap-2">
                <LoadingDots />
                {busyLabel}
              </span>
            ) : (
              '🚀 生成'
            )}
          </button>
        </div>
      </div>

      {hasApiKey && (
        <button
          type="button"
          onClick={() => setSettingsOpen(true)}
          className="text-[11px] text-white/35 hover:text-white/60 transition-colors"
        >
          {settings.model_image_path
            ? '✓ 見本画像を設定済み — 構図・配色・文字配置を踏襲したプロンプトを作ります'
            : '見本画像が未設定です（設定するとデザインを踏襲したプロンプトになります）→ 設定'}
        </button>
      )}

      {(error || pickError) && (
        <div className="w-full max-w-2xl flex items-start gap-2.5 px-4 py-3 rounded-xl bg-red-500/10 border border-red-500/20 text-red-300 text-[12px]">
          <span className="shrink-0 mt-0.5">✕</span>
          <span>{pickError ?? error?.message}</span>
        </div>
      )}

      {hasApiKey && catalogStatus === 'loading' && (
        <div className="flex items-center gap-2 px-4 py-2 rounded-xl bg-white/5 border border-white/10 text-white/50 text-[12px]">
          <LoadingDots />
          <span>公開モデルを取得し、安定版（最新の1世代前）を自動選択しています…</span>
        </div>
      )}

      {hasApiKey && modelsReady && activeModelId && (
        <p className="text-[11px] text-white/35">
          実行モデル: {activeModelId}
          {usedFallback && <span className="text-yellow-300/70">（混雑のため自動切替）</span>}
        </p>
      )}

      {hasApiKey && catalogError && (
        <p className="text-[11px] text-yellow-300/80">モデル一覧の取得に失敗しました。生成時に再解決します。</p>
      )}

      {!source && !error && (
        <p className="text-[11px] text-white/20 text-center leading-relaxed">
          素材を選んで「生成」を押すと<br />
          サムネイル用プロンプト3パターンと、SEOタイトル・概要欄・ハッシュタグ・チャプターを生成します
        </p>
      )}
    </div>
  );
}

function FileSlot({
  file,
  hint,
  disabled,
  onPick,
  onClear,
}: {
  file?: PickedFile;
  hint: string;
  disabled: boolean;
  onPick: () => void;
  onClear: () => void;
}) {
  if (!file) {
    return (
      <button
        type="button"
        onClick={onPick}
        disabled={disabled}
        className="flex flex-col items-center justify-center gap-1.5 h-32 rounded-xl border border-dashed border-white/15 bg-white/[0.02] hover:bg-white/[0.05] hover:border-accent/40 transition-colors disabled:opacity-35 disabled:cursor-not-allowed"
      >
        <span className="text-[13px] font-semibold text-white/70">ファイルを選択…</span>
        <span className="text-[11px] text-white/30">{hint}</span>
      </button>
    );
  }
  return (
    <div className="flex items-center gap-3 h-32 px-5 rounded-xl border border-accent/30 bg-accent/5">
      <div className="flex-1 min-w-0">
        <p className="text-[13px] font-semibold text-white truncate" title={file.path}>
          {file.name}
        </p>
        <p className="text-[11px] text-white/35 mt-0.5">{formatBytes(file.size)}</p>
      </div>
      <button
        type="button"
        onClick={onPick}
        disabled={disabled}
        className="px-3 py-1.5 rounded-lg bg-white/8 hover:bg-white/14 text-[11px] text-white/70 disabled:opacity-35"
      >
        変更
      </button>
      <button
        type="button"
        onClick={onClear}
        disabled={disabled}
        className="px-2 py-1.5 rounded-lg text-[11px] text-white/40 hover:text-white/70 disabled:opacity-35"
        aria-label="選択を解除"
      >
        ✕
      </button>
    </div>
  );
}

function formatBytes(n: number): string {
  if (n >= 1024 * 1024 * 1024) return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n >= 1024) return `${Math.round(n / 1024)} KB`;
  return `${n} B`;
}

function LoadingDots() {
  return (
    <span className="flex gap-0.5 items-center">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="w-1 h-1 rounded-full bg-white animate-bounce"
          style={{ animationDelay: `${i * 0.15}s` }}
        />
      ))}
    </span>
  );
}

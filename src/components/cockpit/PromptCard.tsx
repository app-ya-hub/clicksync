import { useState } from 'react';
import type { ThumbnailPrompt } from '../../types';
import { CopyButton } from '../common/CopyButton';

export function PromptCard({ spec }: { spec: ThumbnailPrompt }) {
  const [expanded, setExpanded] = useState(false);

  const details: [string, string][] = [
    ['表情・ポーズ', spec.expression],
    ['配色', spec.color_scheme],
    ['文字装飾', spec.decoration],
    ['背景・エフェクト', spec.background],
  ];

  return (
    <section className="rounded-2xl border border-white/8 bg-surface-elevated/60 overflow-hidden">
      <header className="flex items-center gap-3 px-4 py-3 border-b border-white/6">
        <span className="shrink-0 px-2 py-0.5 rounded-md bg-accent/15 text-accent text-[11px] font-bold">
          パターン{spec.pattern_id}
        </span>
        <p className="flex-1 min-w-0 text-[12px] text-white/70 truncate select-text" title={spec.angle}>
          {spec.angle}
        </p>
        <CopyButton
          text={spec.prompt}
          label={`📋 パターン${spec.pattern_id}をコピー`}
          variant="solid"
        />
      </header>

      <div className="px-4 py-3 flex flex-col gap-3">
        <div className="flex flex-wrap gap-2">
          {spec.killer_words.map((w, i) => (
            <span
              key={i}
              className="px-2.5 py-1 rounded-lg bg-white/8 border border-white/10 text-[15px] font-black tracking-tight select-text"
            >
              {w}
            </span>
          ))}
        </div>

        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-[11px]">
          {details
            .filter(([, v]) => v.trim())
            .map(([label, value]) => (
              <div key={label} className="contents">
                <dt className="text-white/35 whitespace-nowrap">{label}</dt>
                <dd className="text-white/65 leading-relaxed select-text">{value}</dd>
              </div>
            ))}
        </dl>

        <div>
          <button
            type="button"
            onClick={() => setExpanded((v) => !v)}
            className="text-[10px] text-white/35 hover:text-white/70 transition-colors"
          >
            {expanded ? '▲ プロンプトを閉じる' : '▼ プロンプト全文を表示'}
          </button>
          {expanded && (
            <pre className="mt-2 max-h-80 overflow-auto rounded-lg bg-black/30 border border-white/6 p-3 text-[11px] leading-relaxed text-white/75 whitespace-pre-wrap font-sans select-text">
              {spec.prompt}
            </pre>
          )}
        </div>
      </div>
    </section>
  );
}

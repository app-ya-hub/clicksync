import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { GeneratedAssets } from '../../types';
import { useAppStore } from '../../stores/appStore';
import { RegenerateButton } from '../actions/RegenerateButton';
import { PromptCard } from './PromptCard';
import { SeoPanel } from './SeoPanel';

export function CockpitWorkspace({ assets }: { assets: GeneratedAssets }) {
  const { settings, setSettingsOpen, status, error } = useAppStore();
  const usedReference = !!assets.reference_layout;
  const referencePreview = useReferencePreview(usedReference ? settings.model_image_path : null);

  return (
    <div className="flex flex-col flex-1 min-h-0">
      <div className="flex items-center gap-3 px-6 py-3 border-b border-white/8 shrink-0">
        <div className="flex-1 min-w-0">
          <p className="text-[10px] text-white/30 truncate">{assets.source_label}</p>
          <p className="text-[13px] font-semibold truncate select-text" title={assets.title}>
            {assets.title || '（テーマ未入力）'}
          </p>
        </div>
        {assets.tone.category && (
          <span
            className="shrink-0 px-2.5 py-1 rounded-full bg-purple-500/15 border border-purple-500/30 text-purple-200 text-[11px] font-semibold"
            title={assets.tone.summary}
          >
            トーン: {assets.tone.category}
          </span>
        )}
        <RegenerateButton compact />
      </div>
      {status === 'error' && error && (
        <p className="px-6 py-2 text-[11px] text-red-300 bg-red-500/10 border-b border-red-500/20 shrink-0">
          再生成できませんでした: {error.message}
        </p>
      )}

      <div className="flex flex-1 min-h-0">
        <div className="flex-1 min-w-0 overflow-y-auto px-6 py-4 flex flex-col gap-3">
          <div className="flex items-center gap-3 rounded-xl border border-accent/20 bg-accent/5 px-3 py-2.5">
            {referencePreview && (
              <img
                src={referencePreview}
                alt="見本画像"
                className="w-20 aspect-video rounded-md object-cover border border-white/10 shrink-0"
              />
            )}
            <div className="flex-1 min-w-0 text-[11px] leading-relaxed">
              <p className="text-white/80 font-semibold">サムネイル用プロンプト（Web版Gemini向け）</p>
              {usedReference ? (
                <p className="text-white/45">
                  Web版Geminiで見本画像を添付し、コピーしたプロンプトを貼り付けて送信してください。
                </p>
              ) : (
                <p className="text-white/45">
                  見本画像が未設定のため、トーンに合わせたデザインで作成しています。
                  <button
                    type="button"
                    onClick={() => setSettingsOpen(true)}
                    className="ml-1 text-accent hover:underline"
                  >
                    見本画像を設定する
                  </button>
                </p>
              )}
            </div>
          </div>

          {assets.tone.summary && (
            <p className="text-[11px] text-white/40 leading-relaxed select-text">{assets.tone.summary}</p>
          )}

          {assets.thumbnail_prompts.map((spec) => (
            <PromptCard key={spec.pattern_id} spec={spec} />
          ))}
        </div>

        <aside className="w-[420px] shrink-0 border-l border-white/8 overflow-y-auto px-4 py-4">
          <SeoPanel seo={assets.seo} />
        </aside>
      </div>
    </div>
  );
}

function useReferencePreview(path: string | null): string | null {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!path) {
      setUrl(null);
      return;
    }
    let cancelled = false;
    invoke<string | null>('cmd_read_image_data_url', { path })
      .then((u) => !cancelled && setUrl(u ?? null))
      .catch(() => !cancelled && setUrl(null));
    return () => {
      cancelled = true;
    };
  }, [path]);
  return url;
}

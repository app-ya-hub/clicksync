import { useGenerate } from '../../hooks/useGenerate';
import { useAppStore } from '../../stores/appStore';

interface RegenerateButtonProps {
  /** true = アクションバー用の小さな表示 */
  compact?: boolean;
  disabled?: boolean;
}

/** 読み込み済みの素材と設定を維持したまま、Gemini へ再リクエストして全項目を作り直す */
export function RegenerateButton({ compact, disabled }: RegenerateButtonProps) {
  const { regenerate, status } = useGenerate();
  const hasSource = useAppStore((s) => !!s.videoData);

  const running = status === 'fetching' || status === 'generating';
  const isDisabled = disabled || running || !hasSource;

  const label = running
    ? status === 'fetching'
      ? '素材を読み込み中…'
      : 'AI再生成中…'
    : '🔄 再生成';

  const size = compact
    ? 'px-3 py-1.5 rounded-lg text-[11px]'
    : 'px-4 py-2.5 rounded-xl text-[12px]';

  return (
    <button
      type="button"
      onClick={() => void regenerate()}
      disabled={isDisabled}
      title={
        hasSource
          ? '同じ素材・設定のまま、サムネイル用プロンプトとSEOメタデータをすべて作り直します（文字起こしはやり直しません）'
          : '素材が保持されていないため再生成できません（新規作成から読み込み直してください）'
      }
      className={`flex items-center justify-center gap-2 font-bold border transition-all bg-white/8 hover:bg-white/14 text-white/85 border-white/10 disabled:opacity-35 disabled:cursor-not-allowed ${size}`}
    >
      {running && (
        <span className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
      )}
      {label}
    </button>
  );
}

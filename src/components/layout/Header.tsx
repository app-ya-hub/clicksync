import { useAppStore } from '../../stores/appStore';
import { useSettings } from '../../hooks/useSettings';
import { useGeminiCatalog } from '../../hooks/useGeminiCatalog';

export function Header() {
  const { status, assets, reset } = useAppStore();
  const { hasApiKey, setSettingsOpen } = useSettings();
  const { catalogStatus, activeModelId } = useGeminiCatalog();

  return (
    <header className="flex items-center justify-between px-6 h-14 border-b border-white/8 bg-surface shrink-0 z-10">
      {/* Logo */}
      <button
        onClick={assets ? reset : undefined}
        className="flex items-center gap-2 group"
      >
        <span className="text-[15px] font-bold tracking-tight leading-none">
          Click<span className="text-accent">Sync</span>
        </span>
        {assets && (
          <span className="text-[10px] text-white/30 group-hover:text-white/50 transition-colors mt-[1px]">
            ← 新規
          </span>
        )}
      </button>

      {/* Center: pipeline status */}
      {catalogStatus === 'loading' && status === 'idle' && (
        <StatusPill color="blue" text="モデルを自動選択中…" pulse />
      )}
      {status === 'idle' && catalogStatus === 'ready' && activeModelId && (
        <StatusPill color="green" text={activeModelId} />
      )}
      {status === 'fetching' && (
        <StatusPill color="blue" text="素材を読み込み・文字起こし中…" pulse />
      )}
      {status === 'generating' && (
        <StatusPill color="purple" text="AI生成中 (Gemini)…" pulse />
      )}
      {status === 'done' && (
        <StatusPill color="green" text="生成完了" />
      )}

      {/* Right: settings */}
      <button
        onClick={() => setSettingsOpen(true)}
        className="flex items-center gap-1.5 text-[12px] text-white/40 hover:text-white/80 transition-colors"
      >
        <span className="text-base leading-none">⚙</span>
        <span>設定</span>
        {!hasApiKey && (
          <span className="w-1.5 h-1.5 rounded-full bg-yellow-400 animate-pulse" />
        )}
      </button>
    </header>
  );
}

function StatusPill({
  color,
  text,
  pulse,
}: {
  color: 'blue' | 'purple' | 'green';
  text: string;
  pulse?: boolean;
}) {
  const colors = {
    blue: 'bg-blue-500/15 text-blue-300 border-blue-500/30',
    purple: 'bg-purple-500/15 text-purple-300 border-purple-500/30',
    green: 'bg-emerald-500/15 text-emerald-300 border-emerald-500/30',
  };
  return (
    <div
      className={`flex items-center gap-1.5 px-3 py-1 rounded-full border text-[11px] font-medium ${colors[color]}`}
    >
      {pulse && (
        <span className="w-1.5 h-1.5 rounded-full bg-current animate-pulse" />
      )}
      {text}
    </div>
  );
}

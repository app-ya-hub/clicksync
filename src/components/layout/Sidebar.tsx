import { useAppStore } from '../../stores/appStore';

export function Sidebar() {
  const { history, assets, setAssets, setVideoData } = useAppStore();

  if (history.length === 0) return null;

  return (
    <aside className="w-56 shrink-0 border-r border-white/8 bg-surface overflow-y-auto flex flex-col">
      <div className="px-4 py-3 border-b border-white/8">
        <span className="text-[10px] font-semibold uppercase tracking-widest text-white/30">
          生成履歴
        </span>
      </div>
      <div className="flex flex-col gap-1 p-2">
        {history.map((entry) => (
          <button
            key={entry.id}
            onClick={() => {
              setVideoData(entry.video_data);
              setAssets(entry.assets);
            }}
            className={`text-left px-3 py-2.5 rounded-lg transition-colors group ${
              assets?.generated_at === entry.generated_at
                ? 'bg-white/10 text-white'
                : 'text-white/50 hover:bg-white/5 hover:text-white/80'
            }`}
          >
            <p className="text-[11px] font-medium leading-snug line-clamp-2">
              {entry.title || entry.assets.source_label}
            </p>
            <p className="text-[10px] text-white/30 mt-1">
              {new Date(entry.generated_at).toLocaleString('ja-JP', {
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
              })}
            </p>
          </button>
        ))}
      </div>
    </aside>
  );
}

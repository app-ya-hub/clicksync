import { useEffect, useRef, useState } from 'react';

type CopyState = 'idle' | 'copied' | 'error';

interface CopyButtonProps {
  text: string;
  /** 通常時のラベル。省略時は「コピー」 */
  label?: string;
  /** コピー完了時のラベル。省略時は「✓ コピー完了！」 */
  copiedLabel?: string;
  /** ghost = 一覧内の小さなボタン / solid = 主要アクション */
  variant?: 'ghost' | 'solid';
  className?: string;
}

/** クリップボードへコピーし、一時的に「コピー完了！」へ切り替わるボタン */
export function CopyButton({
  text,
  label,
  copiedLabel = '✓ コピー完了！',
  variant = 'ghost',
  className = '',
}: CopyButtonProps) {
  const [state, setState] = useState<CopyState>('idle');
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  const handleCopy = async () => {
    const ok = await copyToClipboard(text);
    setState(ok ? 'copied' : 'error');
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setState('idle'), 1600);
  };

  const base =
    variant === 'solid'
      ? 'px-3 py-1.5 rounded-lg text-[11px] font-semibold border'
      : 'px-2 py-1 rounded-md text-[10px] font-semibold border';

  const tone =
    state === 'copied'
      ? 'bg-emerald-500/20 text-emerald-300 border-emerald-500/30'
      : state === 'error'
      ? 'bg-red-500/15 text-red-300 border-red-500/30'
      : variant === 'solid'
      ? 'bg-white/8 text-white/70 border-white/10 hover:bg-white/14 hover:text-white'
      : 'bg-white/8 text-white/40 border-transparent hover:bg-white/12 hover:text-white/70';

  return (
    <button
      type="button"
      onClick={handleCopy}
      disabled={!text}
      title={text ? undefined : 'コピーする内容がありません'}
      className={`shrink-0 whitespace-nowrap transition-all disabled:opacity-30 disabled:cursor-not-allowed ${base} ${tone} ${className}`}
    >
      {state === 'copied' ? copiedLabel : state === 'error' ? '✕ 失敗' : label ?? 'コピー'}
    </button>
  );
}

/** navigator.clipboard が使えない環境向けのフォールバック付きコピー */
async function copyToClipboard(text: string): Promise<boolean> {
  if (!text) return false;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    try {
      const el = document.createElement('textarea');
      el.value = text;
      el.setAttribute('readonly', '');
      el.style.position = 'fixed';
      el.style.opacity = '0';
      document.body.appendChild(el);
      el.select();
      const ok = document.execCommand('copy');
      document.body.removeChild(el);
      return ok;
    } catch {
      return false;
    }
  }
}

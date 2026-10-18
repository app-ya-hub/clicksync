import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AboutInfo {
  version: string;
  third_party_licenses: string;
}

/** 設定画面下部の「バージョン・著作権・ライセンス情報」 */
export function AboutSection() {
  const [about, setAbout] = useState<AboutInfo | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    invoke<AboutInfo>('cmd_about').then(setAbout).catch(() => setAbout(null));
  }, []);

  return (
    <>
      <div className="flex flex-col gap-0.5 text-[10px] text-white/30">
        <span>ClickSync {about ? `v${about.version}` : ''} © 2026 ClickSync. All rights reserved.</span>
        <button
          type="button"
          onClick={() => setOpen(true)}
          disabled={!about}
          className="text-left text-white/40 hover:text-white/70 underline-offset-2 hover:underline disabled:opacity-40"
        >
          オープンソースライセンス
        </button>
      </div>

      {open && about && (
        <div
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/70"
          onClick={(e) => e.target === e.currentTarget && setOpen(false)}
        >
          <div className="bg-[#1a1a1a] border border-white/10 rounded-2xl w-full max-w-2xl mx-4 flex flex-col shadow-2xl overflow-hidden h-[80vh]">
            <div className="flex items-center justify-between px-5 py-3 border-b border-white/8 shrink-0">
              <span className="font-bold text-[13px]">オープンソースライセンス</span>
              <button
                onClick={() => setOpen(false)}
                className="text-white/30 hover:text-white/70 transition-colors text-lg leading-none"
              >
                ✕
              </button>
            </div>
            <pre className="flex-1 overflow-auto p-5 text-[10px] leading-relaxed text-white/60 whitespace-pre-wrap font-mono select-text">
              {about.third_party_licenses}
            </pre>
          </div>
        </div>
      )}
    </>
  );
}

import type { ReactNode } from 'react';
import type { SeoMetadata } from '../../types';
import { buildFullDescription, chaptersToText, hashtagsToText } from '../../utils/seo';
import { CopyButton } from '../common/CopyButton';

export function SeoPanel({ seo }: { seo: SeoMetadata }) {
  const fullDescription = buildFullDescription(seo);

  return (
    <div className="flex flex-col gap-3">
      <Card
        title="SEOタイトル"
        action={<CopyButton text={seo.title} label="📋 タイトルをコピー" variant="solid" />}
      >
        <p className="text-[14px] font-bold leading-snug select-text">{seo.title}</p>
        <p className="text-[10px] text-white/30 mt-1">{[...seo.title].length}文字</p>

        {seo.alt_titles.length > 0 && (
          <div className="mt-3 flex flex-col gap-1.5">
            <p className="text-[10px] text-white/35">別案</p>
            {seo.alt_titles.map((t, i) => (
              <div key={i} className="flex items-start gap-2">
                <p className="flex-1 text-[12px] text-white/65 leading-snug select-text">{t}</p>
                <CopyButton text={t} />
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card
        title="概要欄"
        action={<CopyButton text={fullDescription} label="📋 概要欄をコピー" variant="solid" />}
      >
        <pre className="max-h-72 overflow-auto text-[11px] leading-relaxed text-white/70 whitespace-pre-wrap font-sans select-text">
          {fullDescription}
        </pre>
        <div className="mt-2 flex justify-end">
          <CopyButton text={seo.description_body} label="本文のみコピー" />
        </div>
      </Card>

      <Card
        title="ハッシュタグ"
        action={<CopyButton text={hashtagsToText(seo.hashtags)} label="📋 コピー" variant="solid" />}
      >
        <div className="flex flex-wrap gap-1.5">
          {seo.hashtags.map((h) => (
            <span key={h} className="px-2 py-0.5 rounded-md bg-blue-500/10 text-blue-300 text-[12px] select-text">
              {h}
            </span>
          ))}
        </div>
      </Card>

      <Card
        title="チャプター"
        action={<CopyButton text={chaptersToText(seo.chapters)} label="📋 コピー" variant="solid" />}
      >
        {seo.chapters.length === 0 ? (
          <p className="text-[11px] text-white/35">素材に時刻情報が無いため、チャプターは作成していません（音声・字幕ファイルなら作成できます）</p>
        ) : (
          <ul className="flex flex-col gap-1 select-text">
            {seo.chapters.map((c) => (
              <li key={c.start_seconds} className="flex gap-3 text-[12px]">
                <span className="font-mono text-accent/90 shrink-0">{c.timestamp}</span>
                <span className="text-white/70">{c.title}</span>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card
        title="タグ（検索キーワード）"
        action={<CopyButton text={seo.keywords.join(',')} label="📋 コピー" variant="solid" />}
      >
        <div className="flex flex-wrap gap-1.5">
          {seo.keywords.map((k) => (
            <span key={k} className="px-2 py-0.5 rounded-md bg-white/6 text-white/60 text-[11px] select-text">
              {k}
            </span>
          ))}
        </div>
      </Card>
    </div>
  );
}

function Card({ title, action, children }: { title: string; action: ReactNode; children: ReactNode }) {
  return (
    <section className="rounded-2xl border border-white/8 bg-surface-elevated/60 px-4 py-3">
      <header className="flex items-center justify-between gap-2 mb-2.5">
        <h3 className="text-[11px] font-semibold text-white/45 tracking-wide">{title}</h3>
        {action}
      </header>
      {children}
    </section>
  );
}

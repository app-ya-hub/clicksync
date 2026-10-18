import type { Chapter, SeoMetadata } from '../types';

/** YouTube の概要欄にそのまま貼れるチャプター行（"00:00 見出し"） */
export function chaptersToText(chapters: Chapter[]): string {
  return chapters.map((c) => `${c.timestamp} ${c.title}`).join('\n');
}

export function hashtagsToText(hashtags: string[]): string {
  return hashtags.join(' ');
}

/** 本文 → チャプター → 定型文 → ハッシュタグ の順で組み立てた概要欄全文 */
export function buildFullDescription(seo: SeoMetadata): string {
  const sections = [
    seo.description_body.trim(),
    seo.chapters.length > 0 ? `▼目次\n${chaptersToText(seo.chapters)}` : '',
    seo.fixed_boilerplate.trim(),
    hashtagsToText(seo.hashtags),
  ];
  return sections.filter(Boolean).join('\n\n');
}

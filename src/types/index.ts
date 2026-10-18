// ─── Enums ────────────────────────────────────────────────────────────────────

export type GenerationStatus = 'idle' | 'fetching' | 'generating' | 'done' | 'error';

// ─── Source material ──────────────────────────────────────────────────────────

export type SourceKind = 'audio' | 'subtitle' | 'text';

/** 生成の素材。音声・字幕はローカルファイルの絶対パス、台本は本文そのもの */
export type SourceInput =
  | { kind: 'audio'; path: string }
  | { kind: 'subtitle'; path: string }
  | { kind: 'text'; text: string };

export interface PickedFile {
  path: string;
  name: string;
  size: number;
}

export interface VideoMeta {
  /** テーマ・仮タイトル（未入力ならファイル名） */
  title: string;
  /** 「音声ファイル: xxx.mp3」などの素材表示 */
  source_label: string;
  /** 不明なら 0 */
  duration_seconds: number;
}

export interface CaptionSegment {
  start_seconds: number;
  duration_seconds: number;
  text: string;
}

export interface VideoData {
  meta: VideoMeta;
  captions: CaptionSegment[];
  full_transcript: string;
  /** 時刻情報があるか（台本テキストのみの場合は false で、チャプターは作らない） */
  has_captions: boolean;
}

// ─── Generation output ────────────────────────────────────────────────────────

/** 文字起こしから判定した動画のトーン（分類名は動画ごとに AI が付ける） */
export interface ToneProfile {
  category: string;
  summary: string;
}

/** 1パターン分のサムネイル設計と、Web版Geminiへ貼り付ける完成プロンプト */
export interface ThumbnailPrompt {
  /** "1" / "2" / "3" */
  pattern_id: string;
  angle: string;
  /** サムネに入れる文字（1〜3ブロック） */
  killer_words: string[];
  expression: string;
  color_scheme: string;
  decoration: string;
  background: string;
  prompt: string;
}

export interface Chapter {
  timestamp: string;
  start_seconds: number;
  title: string;
}

export interface SeoMetadata {
  title: string;
  alt_titles: string[];
  description_body: string;
  fixed_boilerplate: string;
  /** タグ欄用の検索キーワード */
  keywords: string[];
  hashtags: string[];
  chapters: Chapter[];
}

export interface TextColorStyle {
  fill: string;
  stroke: string;
  outer?: string | null;
}

/** 見本画像の解析結果（構図・配色・文字配置） */
export interface ReferenceLayout {
  block_count: number;
  block_chars: number[];
  total_chars: number;
  block_roles: string[];
  character_area: string;
  text_area: string;
  art_style: string;
  subject_description: string;
  background_effects: string;
  color_tone: string;
  text_decoration: string;
  composition_summary: string;
  palette: string[];
  text_styles: TextColorStyle[];
  analyzed_by_ai: boolean;
}

export interface GeneratedAssets {
  title: string;
  source_label: string;
  generated_at: string;
  tone: ToneProfile;
  thumbnail_prompts: ThumbnailPrompt[];
  seo: SeoMetadata;
  reference_layout?: ReferenceLayout | null;
}

// ─── Settings ─────────────────────────────────────────────────────────────────

export interface AppSettings {
  gemini_api_key: string;
  gemini_model: string;
  fixed_boilerplate: string;
  model_image_path: string | null;
}

export interface GeminiModelInfo {
  id: string;
  display_name: string;
  recommended_default: boolean;
}

export interface GeminiModelCatalog {
  models: GeminiModelInfo[];
  /** Flash系の実行候補。先頭が最新で、後ろほど古い世代 */
  candidates: string[];
  auto_selected_index: number;
  auto_selected_id: string;
  auto_selected_display_name: string;
}

// ─── UI State ─────────────────────────────────────────────────────────────────

export interface HistoryEntry {
  id: string;
  title: string;
  generated_at: string;
  assets: GeneratedAssets;
  /** 再生成に使う素材（文字起こし済み） */
  video_data: VideoData;
}

export interface AppError {
  message: string;
  code?: string;
}

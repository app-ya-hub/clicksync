use serde::{Deserialize, Serialize};

// ─── Source material ─────────────────────────────────────────────────────────

/// 生成の元になる素材（YouTube にはアクセスしない）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceInput {
    /// 音声ファイル（Gemini で文字起こし）
    Audio { path: String },
    /// 字幕ファイル（SRT / VTT / SBV）
    Subtitle { path: String },
    /// 台本・テキスト（時刻情報なし）
    Text { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMeta {
    /// 利用者が入力したテーマ・仮タイトル（未入力ならファイル名）
    pub title: String,
    /// 「音声ファイル: talk.mp3」など、素材の説明
    pub source_label: String,
    /// 不明なら 0
    pub duration_seconds: u64,
}

/// A single caption segment with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionSegment {
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub text: String,
}

/// 素材を読み込んだ結果。生成・再生成はこれを使い回す（音声の文字起こしをやり直さない）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoData {
    pub meta: VideoMeta,
    pub captions: Vec<CaptionSegment>,
    pub full_transcript: String,
    /// 時刻情報がある（字幕・音声）。false ならチャプターを作らない
    pub has_captions: bool,
}

// ─── Generation output ───────────────────────────────────────────────────────

/// 生成結果一式（フロントへ返す）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedAssets {
    pub title: String,
    pub source_label: String,
    pub generated_at: String, // ISO8601
    /// 文字起こしから判定した動画のトーン
    pub tone: ToneProfile,
    /// Web版Gemini向けのサムネイル画像生成プロンプト（パターン1・2・3）
    pub thumbnail_prompts: Vec<ThumbnailPrompt>,
    pub seo: SeoMetadata,
    /// 見本画像の解析結果（見本未設定なら None）
    #[serde(default)]
    pub reference_layout: Option<ReferenceLayout>,
}

/// 動画のトーン判定（分類名は動画ごとに AI が付ける）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToneProfile {
    pub category: String,
    pub summary: String,
}

/// 1パターン分のサムネイル設計と、Web版Geminiへ貼り付ける完成プロンプト
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailPrompt {
    /// "1" / "2" / "3"
    pub pattern_id: String,
    /// このパターンの切り口
    pub angle: String,
    /// サムネに入れる文字（1〜3ブロック）
    pub killer_words: Vec<String>,
    /// 人物・キャラの表情とポーズ
    pub expression: String,
    pub color_scheme: String,
    pub decoration: String,
    pub background: String,
    /// そのままコピーして使う画像生成プロンプト
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoMetadata {
    /// クリック率重視のメインタイトル
    pub title: String,
    /// 別案タイトル
    pub alt_titles: Vec<String>,
    /// 検索キーワードを網羅した概要欄本文
    pub description_body: String,
    /// 設定の固定定型文（概要欄の末尾に入れる）
    pub fixed_boilerplate: String,
    /// タグ欄用の検索キーワード
    pub keywords: Vec<String>,
    /// 3〜5個
    pub hashtags: Vec<String>,
    pub chapters: Vec<Chapter>,
}

/// One YouTube chapter entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Chapter {
    pub timestamp: String, // "00:00"
    pub start_seconds: u64,
    pub title: String,
}

pub const MIN_TELOP_BLOCKS: usize = 1;
pub const MAX_TELOP_BLOCKS: usize = 3;
/// 見本の文字量に対して AI に許す裁量（±30%）
pub const TELOP_CHAR_TOLERANCE: f32 = 0.30;
/// 見本が無いときの目安
pub const DEFAULT_TARGET_CHARS: usize = 12;

/// 目安文字数 ±30% の許容範囲（見本なしでも極端な長文だけは弾く）
pub fn telop_char_range(target: Option<usize>) -> (usize, usize) {
    let t = target.unwrap_or(DEFAULT_TARGET_CHARS).max(1) as f32;
    let min = (t * (1.0 - TELOP_CHAR_TOLERANCE)).floor().max(1.0) as usize;
    let max = (t * (1.0 + TELOP_CHAR_TOLERANCE)).ceil() as usize;
    if target.is_some() {
        (min, max.max(min))
    } else {
        (1, max.max(20))
    }
}

/// 見本画像（デザイン見本）を解析した構造。プロンプトの「見本の踏襲」部分に使う。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReferenceLayout {
    /// 文字のブロック数（1〜3）
    pub block_count: usize,
    /// 各ブロックの文字数（見本の実測）
    pub block_chars: Vec<usize>,
    pub total_chars: usize,
    /// 各ブロックの役割
    #[serde(default)]
    pub block_roles: Vec<String>,
    /// 人物・キャラの配置エリア: left / right / center / none
    pub character_area: String,
    /// 文字の配置エリア: left / right / top / bottom / center
    pub text_area: String,
    /// 画風（実写・イラスト・アニメ調など）
    #[serde(default)]
    pub art_style: String,
    /// 人物・キャラの様子（大きさ・表情・ポーズ・視線）
    #[serde(default)]
    pub subject_description: String,
    #[serde(default)]
    pub background_effects: String,
    #[serde(default)]
    pub color_tone: String,
    /// 文字装飾（太さ・フチ・座布団・吹き出し・影など）
    #[serde(default)]
    pub text_decoration: String,
    #[serde(default)]
    pub composition_summary: String,
    /// 背景の主要色 (#RRGGBB)
    #[serde(default)]
    pub palette: Vec<String>,
    /// 文字の配色ルール
    #[serde(default)]
    pub text_styles: Vec<TextColorStyle>,
    /// Gemini で解析できたか（false = ローカル解析のみ）
    #[serde(default)]
    pub analyzed_by_ai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextColorStyle {
    pub fill: String,
    pub stroke: String,
    #[serde(default)]
    pub outer: Option<String>,
}

// ─── Settings ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub gemini_api_key: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    pub fixed_boilerplate: String,
    /// 設定画面の「参考サムネイル」（デザイン見本）。アバター合成には使わない。
    pub model_image_path: Option<String>,
}

fn default_gemini_model() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModelInfo {
    pub id: String,
    pub display_name: String,
    pub recommended_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiModelCatalog {
    pub models: Vec<GeminiModelInfo>,
    /// Flash 系の実行候補。先頭が最新で、後ろほど古い世代。
    pub candidates: Vec<String>,
    /// 既定の開始インデックス（最新の1世代前）。
    pub auto_selected_index: usize,
    pub auto_selected_id: String,
    pub auto_selected_display_name: String,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ClickSyncError {
    #[error("素材を読み込めませんでした: {0}")]
    SourceReadFailed(String),

    #[error("音声の文字起こしに失敗しました: {0}")]
    TranscriptionFailed(String),

    #[error("Gemini API キーが設定されていません")]
    ApiKeyMissing,

    #[error("利用可能なGeminiモデルが見つかりませんでした")]
    NoModelsAvailable,

    #[error("Gemini API エラー ({status}): {message}")]
    GeminiApiError { status: u16, message: String },

    #[error("{}", busy_message(*status))]
    GeminiBusy { status: u16 },

    #[error("Gemini レスポンスの解析に失敗しました: {0}")]
    GeminiParseFailed(String),

    #[error("生成制約違反: {0}")]
    ConstraintViolation(String),

    #[error("ネットワークエラー: {0}")]
    NetworkError(String),

    #[error("見本画像を読み込めませんでした: {0}")]
    ReferenceImageFailed(String),

    #[error("ファイル保存エラー: {0}")]
    CacheError(String),
}

fn busy_message(status: u16) -> String {
    if status == 429 {
        "Gemini API の利用上限（1分あたり・1日あたりの回数）に達しました（429）。しばらく待ってから再度お試しください。\
         続く場合は Google AI Studio で利用状況・課金設定を確認してください。"
            .into()
    } else {
        format!(
            "Google の Gemini サーバーが混雑しています（{status}）。APIキーの問題ではありません。\
             自動で待って再試行しましたが空きませんでした。数分おいてから、もう一度お試しください。"
        )
    }
}

// Serialize errors for Tauri IPC
impl Serialize for ClickSyncError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

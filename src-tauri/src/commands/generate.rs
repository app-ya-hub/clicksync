/// generate.rs
///
/// VideoData + AppSettings (+ 見本画像の解析結果) → Gemini → GeneratedAssets
///
///   1. 文字起こしから動画のトーンを判定
///   2. 見本の構図・配色・文字配置を踏襲したサムネイル設計を3パターン作り、
///      Web版Geminiにそのまま貼れる画像生成プロンプトへ組み立てる
///   3. SEOタイトル・概要欄・タグ・ハッシュタグ・チャプターを生成
///
/// 文言はすべて動画と見本から動的に生成し、特定ジャンルの語句は持たない。
/// 目安から外れた出力は理由を添えて1回だけ再生成し、それでも外れたら警告付きで採用する。

use crate::{
    commands::{gemini::call_gemini, reference::analyze_reference},
    types::{
        telop_char_range, AppSettings, CaptionSegment, Chapter, ClickSyncError, GeneratedAssets,
        ReferenceLayout, SeoMetadata, ThumbnailPrompt, ToneProfile, VideoData,
        DEFAULT_TARGET_CHARS, MAX_TELOP_BLOCKS, MIN_TELOP_BLOCKS,
    },
};
use chrono::Utc;
use serde_json::Value;

const PATTERN_COUNT: usize = 3;
const MAX_HASHTAGS: usize = 5;
const MIN_HASHTAGS: usize = 3;
const MAX_KEYWORDS: usize = 15;
/// 文字起こしダイジェストの上限（文字）
const DIGEST_BUDGET_CHARS: usize = 9000;
/// ダイジェスト1行あたりの区切り（秒）
const DIGEST_WINDOW_SECS: u64 = 20;
/// YouTube はチャプター同士が10秒未満だと認識しない
const MIN_CHAPTER_GAP_SECS: u64 = 10;

// ─── Public entry point ───────────────────────────────────────────────────────

pub async fn generate_assets(
    video_data: &VideoData,
    settings: &AppSettings,
) -> Result<GeneratedAssets, ClickSyncError> {
    // 見本画像があれば先に構造解析（失敗してもローカル解析で続行）
    let layout = match settings.model_image_path.as_deref() {
        Some(path) if !path.is_empty() => Some(analyze_reference(&settings.gemini_api_key, path).await),
        _ => None,
    };

    let system_prompt = build_system_prompt();
    let base_prompt = build_user_prompt(video_data, layout.as_ref());

    let mut user_prompt = base_prompt.clone();
    let mut fallback: Option<GeneratedAssets> = None;
    let mut last_err = None;

    for attempt in 0..2u8 {
        let raw_json = call_gemini(&settings.gemini_api_key, &system_prompt, &user_prompt).await?;
        match parse_gemini_response(&raw_json, video_data, settings, layout.as_ref()) {
            Ok((assets, issues)) => {
                if issues.is_empty() || attempt == 1 {
                    if !issues.is_empty() {
                        eprintln!("[generate] accepted with warnings: {issues:?}");
                    }
                    return Ok(assets);
                }
                eprintln!("[generate] retrying: {issues:?}");
                user_prompt = format!(
                    "{base_prompt}\n## 前回の出力の問題（必ず修正すること）\n- {}\n",
                    issues.join("\n- ")
                );
                fallback = Some(assets);
            }
            Err(e) => {
                eprintln!("[generate] parse failed (attempt {attempt}): {e}");
                last_err = Some(e);
            }
        }
    }
    match fallback {
        Some(assets) => Ok(assets),
        None => Err(last_err.unwrap_or_else(|| {
            ClickSyncError::GeminiParseFailed("生成結果を解析できませんでした".into())
        })),
    }
}

// ─── Prompt construction ──────────────────────────────────────────────────────

fn build_system_prompt() -> String {
    r##"あなたはYouTube運用のプロ（サムネイルのアートディレクター兼SEOコンサルタント）です。
与えられた動画情報・文字起こしと、（あれば）見本サムネイルの解析結果をもとに、指定のJSONだけを出力してください（前置き・マークダウン不要）。
動画に無い事実・数字・固有名詞を作らないこと。

## 1. トーン判定（tone）
- 文字起こしの話題・話し方・視聴者に起こしたい感情から、動画のトーンを判定する。
- category: 「感情系」「ビジネス系」「解説系」「エンタメ系」「ハウツー系」などの分類名。どれにも当てはまらなければ内容に合う分類名を付けてよい。
- summary: 判定理由と、サムネイルで狙う感情を1文で。

## 2. サムネイル設計（thumbnail_patterns: ちょうど3つ）
- 3パターンは切り口（angle）を変える。動画のトーンに最も効く3つの切り口を選ぶ（例: 視聴者の本音の代弁、意外性・問いかけ、損失回避・警告、結果・ビフォーアフター、権威・実績 など）。
- killer_words: サムネに入れる文字。1〜3ブロックの配列（ブロック＝画面上で視覚的に分かれる1かたまり）。
  - 文字量とブロック数は、ユーザープロンプトの「キラーワードの文字量の目安」に従う。
  - 1ブロック内に改行を入れない。一瞬で読める常用的な表記にし、難読漢字は使わない。
- expression: 人物・キャラの表情とポーズを具体的に（トーンと切り口に合わせる）。
- color_scheme: 文字と背景の配色を具体的に。見本がある場合は見本の配色ルールを基調に、トーンに合わせて調整する。
- decoration: 文字装飾（太さ、フチの重ね方、座布団（文字の背景帯）、吹き出し、影など）。見本がある場合は見本の装飾を踏襲する。
- background: 背景とエフェクト。見本がある場合は見本のテイストを踏襲する。

## 3. SEOメタデータ（seo）
- title: クリック率重視。検索されやすい主要キーワードを前半に入れ、スマホで切れない長さ（全角28〜40文字目安）。サムネのキラーワードと同じ表現を繰り返さず、サムネの問いを回収する内容にする。
- alt_titles: 切り口の異なる別案タイトルを2つ。
- description: 概要欄の本文。冒頭2行で動画の価値と主要キーワードを伝え、続けて内容の要点を、検索キーワードを自然に網羅しながら書く（300〜600文字、読みやすく改行）。ハッシュタグ・チャプター・定型文は含めない（アプリ側で付ける）。
- keywords: タグ欄用の検索キーワードを8〜15個（関連語・言い換え・ロングテールを含む）。
- hashtags: 関連ハッシュタグを3〜5個（#付き、スペースなし）。
- chapters: 文字起こしのタイムスタンプに基づくチャプターを5〜8個。最初は必ず "00:00"、昇順、動画の長さ以内、間隔は10秒以上。タイトルは内容が分かる短い見出し。

## 出力形式
{
  "tone": { "category": "...", "summary": "..." },
  "thumbnail_patterns": [
    {
      "angle": "...",
      "killer_words": ["...", "..."],
      "expression": "...",
      "color_scheme": "...",
      "decoration": "...",
      "background": "..."
    }
  ],
  "seo": {
    "title": "...",
    "alt_titles": ["...", "..."],
    "description": "...",
    "keywords": ["...", "..."],
    "hashtags": ["#...", "#..."],
    "chapters": [{ "timestamp": "00:00", "title": "..." }]
  }
}"##
    .to_string()
}

fn area_ja(area: &str) -> &'static str {
    match area {
        "left" => "左側",
        "right" => "右側",
        "center" => "中央",
        "top" => "上部",
        "bottom" => "下部",
        _ => "特定の位置なし",
    }
}

/// 見本の解析結果を、テキストモデルへ渡す説明にする
fn build_reference_section(layout: Option<&ReferenceLayout>) -> String {
    let Some(l) = layout else {
        return "## 見本サムネイル\n未設定。トーンに最適なデザインを自由に設計すること。\n".into();
    };
    let mut s = String::from("## 見本サムネイルの解析結果（デザインはこれを踏襲すること）\n");
    let mut push = |label: &str, v: &str| {
        if !v.trim().is_empty() {
            s.push_str(&format!("- {label}: {}\n", v.trim()));
        }
    };
    push("画風", &l.art_style);
    push("構図", &l.composition_summary);
    push("人物・キャラの配置", area_ja(&l.character_area));
    push("人物・キャラの様子", &l.subject_description);
    push("文字の配置エリア", area_ja(&l.text_area));
    push("背景・エフェクト", &l.background_effects);
    push("配色トーン", &l.color_tone);
    push("背景の主要色", &l.palette.join(", "));
    push("文字装飾", &l.text_decoration);
    push("文字の配色", &text_styles_ja(l));
    s
}

fn text_styles_ja(l: &ReferenceLayout) -> String {
    l.text_styles
        .iter()
        .map(|t| match &t.outer {
            Some(o) => format!("塗り{} / フチ{} / 外フチ{}", t.fill, t.stroke, o),
            None => format!("塗り{} / フチ{}", t.fill, t.stroke),
        })
        .collect::<Vec<_>>()
        .join("、")
}

/// 「キラーワードの文字量の目安」セクション
fn build_density_guidance(layout: Option<&ReferenceLayout>) -> String {
    match layout {
        Some(l) => {
            let (min, max) = telop_char_range(Some(l.total_chars));
            let blocks: Vec<String> = l
                .block_chars
                .iter()
                .enumerate()
                .map(|(i, n)| match l.block_roles.get(i).filter(|r| !r.is_empty()) {
                    Some(role) => format!("{role}（{n}文字）"),
                    None => format!("ブロック{}（{n}文字）", i + 1),
                })
                .collect();
            format!(
                "## キラーワードの文字量の目安（見本の情報密度）\n\
                 - 見本の構成: {count}ブロック ＝ {blocks}\n\
                 - 合計文字数: {total}文字を基準に {min}〜{max}文字（±30%）\n\
                 - ブロック数: {MIN_TELOP_BLOCKS}〜{MAX_TELOP_BLOCKS}（見本の{count}ブロックが基準）\n",
                count = l.block_count,
                blocks = blocks.join(" / "),
                total = l.total_chars,
            )
        }
        None => {
            let (_, max) = telop_char_range(None);
            format!(
                "## キラーワードの文字量の目安（見本なし）\n\
                 - ブロック数: {MIN_TELOP_BLOCKS}〜{MAX_TELOP_BLOCKS}\n\
                 - 合計文字数: {DEFAULT_TARGET_CHARS}文字前後を基準に、最大{max}文字まで\n"
            )
        }
    }
}

fn build_user_prompt(video_data: &VideoData, layout: Option<&ReferenceLayout>) -> String {
    let meta = &video_data.meta;
    let title = if meta.title.trim().is_empty() {
        "（未入力 — 内容から判断すること）".to_string()
    } else {
        meta.title.trim().to_string()
    };
    let duration = if meta.duration_seconds > 0 {
        format!("{}秒（{}）", meta.duration_seconds, format_timestamp(meta.duration_seconds))
    } else {
        "不明".into()
    };
    let (heading, chapter_rule) = if video_data.has_captions {
        ("## 動画の内容（[分:秒] 付き）", "")
    } else {
        ("## 動画の内容（台本・テキスト。時刻情報なし）", "\n※時刻情報がないため、seo.chapters は空配列 [] にすること。\n")
    };
    format!(
        r#"## 動画の情報
- テーマ・仮タイトル: {title}
- 素材: {source}
- 動画の長さ: {duration}

{heading}
{transcript}
{chapter_rule}
{reference}
{density}"#,
        source = meta.source_label,
        transcript = transcript_digest(video_data),
        reference = build_reference_section(layout),
        density = build_density_guidance(layout),
    )
}

/// 字幕を約20秒ごとの行にまとめ、タイムスタンプ付きで上限内に収める。
fn transcript_digest(video_data: &VideoData) -> String {
    if !video_data.has_captions || video_data.captions.is_empty() {
        return video_data.full_transcript.trim().chars().take(DIGEST_BUDGET_CHARS).collect();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut start: Option<u64> = None;
    let mut buf = String::new();
    for seg in &video_data.captions {
        let st = seg.start_seconds.max(0.0) as u64;
        let s0 = *start.get_or_insert(st);
        if st >= s0 + DIGEST_WINDOW_SECS && !buf.is_empty() {
            lines.push(format!("[{}] {}", format_timestamp(s0), buf.trim()));
            buf.clear();
            start = Some(st);
        }
        buf.push_str(&seg.text.replace('\n', " "));
        buf.push(' ');
    }
    if let (Some(s0), false) = (start, buf.trim().is_empty()) {
        lines.push(format!("[{}] {}", format_timestamp(s0), buf.trim()));
    }

    let total: usize = lines.iter().map(|l| l.chars().count() + 1).sum();
    let step = total.div_ceil(DIGEST_BUDGET_CHARS).max(1);
    lines.into_iter().step_by(step).collect::<Vec<_>>().join("\n")
}

// ─── Web版Gemini 向けプロンプト ────────────────────────────────────────────────

/// AI が設計した1パターンを、Web版Geminiへそのまま貼れる画像生成プロンプトにする。
pub fn build_web_prompt(
    spec: &PatternSpec,
    layout: Option<&ReferenceLayout>,
    tone: &ToneProfile,
) -> String {
    let mut s = String::from("YouTubeのサムネイル画像を1枚生成してください（横長16:9、1280×720）。\n\n");

    if let Some(l) = layout {
        s.push_str(
            "【見本画像】\n\
             このプロンプトと一緒に添付した見本サムネイルの「構図・配色・文字の配置エリア」を忠実に踏襲してください。\n\
             見本に書かれている文字そのものは使わないでください。\n",
        );
        let mut push = |label: &str, v: &str| {
            if !v.trim().is_empty() {
                s.push_str(&format!("・{label}: {}\n", v.trim()));
            }
        };
        push("画風（見本と同じ表現手法で）", &l.art_style);
        push("構図", &l.composition_summary);
        if l.character_area != "none" {
            push("人物・キャラの配置", &format!("画面の{}", area_ja(&l.character_area)));
        }
        push("文字の配置エリア", &format!("画面の{}", area_ja(&l.text_area)));
        push("配色トーン", &l.color_tone);
        push("主要色", &l.palette.join(", "));
        push("文字の配色", &text_styles_ja(l));
        s.push('\n');
    }

    if !tone.category.trim().is_empty() {
        s.push_str(&format!("【動画のトーン】{}", tone.category.trim()));
        if !tone.summary.trim().is_empty() {
            s.push_str(&format!(" — {}", tone.summary.trim()));
        }
        s.push_str("\n\n");
    }
    if !spec.angle.trim().is_empty() {
        s.push_str(&format!("【このサムネの切り口】{}\n\n", spec.angle.trim()));
    }

    s.push_str("【サムネイルに入れる文字（キラーワード）】\n");
    for (i, w) in spec.killer_words.iter().enumerate() {
        s.push_str(&format!("{}行目:「{}」\n", i + 1, w));
    }
    s.push_str(
        "・上記の文字だけを一字一句正確に、極太の文字で大きく入れてください（誤字・脱字・余計な文字は入れない）。\n",
    );
    if layout.is_some() {
        s.push_str("・見本と同じくらいの文字量・行数感で、見本と同じ配置エリアに収めてください。\n");
    }
    s.push('\n');

    let mut section = |label: &str, v: &str| {
        if !v.trim().is_empty() {
            s.push_str(&format!("【{label}】{}\n", v.trim()));
        }
    };
    section("人物の表情・ポーズ", &spec.expression);
    section("配色", &spec.color_scheme);
    section("文字装飾", &spec.decoration);
    section("背景・エフェクト", &spec.background);

    s.push_str(
        "\n【守ってほしいこと】\n\
         ・右下（再生時間が表示される位置）には文字や重要な要素を置かない\n\
         ・ロゴ・透かし・枠線は入れない\n\
         ・スマホの小さな表示でも文字がはっきり読めるコントラストにする\n",
    );
    s
}

// ─── Response parsing & validation ───────────────────────────────────────────

/// AI が返した1パターン分の設計
#[derive(Debug, Clone, Default)]
pub struct PatternSpec {
    pub angle: String,
    pub killer_words: Vec<String>,
    pub expression: String,
    pub color_scheme: String,
    pub decoration: String,
    pub background: String,
}

fn str_field(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").trim().to_string()
}

fn str_list(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 解析結果と「目安から外れた点」（再生成のヒント）を返す。
/// 構造的に使えない出力（JSON不正・パターン不足・タイトル/キラーワード空）のみエラー。
fn parse_gemini_response(
    raw: &str,
    video_data: &VideoData,
    settings: &AppSettings,
    layout: Option<&ReferenceLayout>,
) -> Result<(GeneratedAssets, Vec<String>), ClickSyncError> {
    let val: Value = serde_json::from_str(raw.trim())
        .map_err(|e| ClickSyncError::GeminiParseFailed(format!("JSON parse: {e}")))?;

    let tone = ToneProfile {
        category: str_field(&val["tone"], "category"),
        summary: str_field(&val["tone"], "summary"),
    };

    let specs: Vec<PatternSpec> = val["thumbnail_patterns"]
        .as_array()
        .into_iter()
        .flatten()
        .take(PATTERN_COUNT)
        .map(|p| PatternSpec {
            angle: str_field(p, "angle"),
            killer_words: parse_killer_words(p),
            expression: str_field(p, "expression"),
            color_scheme: str_field(p, "color_scheme"),
            decoration: str_field(p, "decoration"),
            background: str_field(p, "background"),
        })
        .collect();
    if specs.len() < PATTERN_COUNT {
        return Err(ClickSyncError::GeminiParseFailed(format!(
            "サムネイル設計が{}パターン（{PATTERN_COUNT}パターン必要）",
            specs.len()
        )));
    }
    for (i, spec) in specs.iter().enumerate() {
        if spec.killer_words.is_empty() {
            return Err(ClickSyncError::ConstraintViolation(format!(
                "パターン{}のキラーワードが空です",
                i + 1
            )));
        }
    }

    let seo_val = &val["seo"];
    let title = str_field(seo_val, "title");
    if title.is_empty() {
        return Err(ClickSyncError::ConstraintViolation("SEOタイトルが空です".into()));
    }
    let seo = SeoMetadata {
        title,
        alt_titles: str_list(&seo_val["alt_titles"]).into_iter().take(3).collect(),
        description_body: str_field(seo_val, "description"),
        fixed_boilerplate: settings.fixed_boilerplate.trim().to_string(),
        keywords: dedupe(str_list(&seo_val["keywords"])).into_iter().take(MAX_KEYWORDS).collect(),
        hashtags: normalize_hashtags(str_list(&seo_val["hashtags"])),
        chapters: resolve_chapters(&seo_val["chapters"], video_data),
    };

    let mut issues = Vec::new();
    let target = layout.map(|l| l.total_chars);
    for (i, spec) in specs.iter().enumerate() {
        issues.extend(killer_word_issues(i + 1, &spec.killer_words, target));
    }
    if seo.hashtags.len() < MIN_HASHTAGS {
        issues.push(format!("ハッシュタグが{}個（3〜5個必要）", seo.hashtags.len()));
    }
    if seo.description_body.chars().count() < 150 {
        issues.push("概要欄の本文が短すぎます（300〜600文字）".into());
    }

    let thumbnail_prompts = specs
        .into_iter()
        .enumerate()
        .map(|(i, spec)| ThumbnailPrompt {
            pattern_id: (i + 1).to_string(),
            prompt: build_web_prompt(&spec, layout, &tone),
            angle: spec.angle,
            killer_words: spec.killer_words,
            expression: spec.expression,
            color_scheme: spec.color_scheme,
            decoration: spec.decoration,
            background: spec.background,
        })
        .collect();

    Ok((
        GeneratedAssets {
            title: video_data.meta.title.clone(),
            source_label: video_data.meta.source_label.clone(),
            generated_at: Utc::now().to_rfc3339(),
            tone,
            thumbnail_prompts,
            seo,
            reference_layout: layout.cloned(),
        },
        issues,
    ))
}

/// `killer_words`（配列）を読む。4つ以上は3つ目以降を最後のブロックへまとめる。
fn parse_killer_words(p: &Value) -> Vec<String> {
    let mut blocks: Vec<String> = str_list(&p["killer_words"])
        .iter()
        .flat_map(|s| s.split('\n'))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if blocks.len() > MAX_TELOP_BLOCKS {
        let tail: String = blocks.split_off(MAX_TELOP_BLOCKS - 1).concat();
        blocks.push(tail);
    }
    blocks
}

fn killer_word_issues(pattern_no: usize, words: &[String], target: Option<usize>) -> Vec<String> {
    let total: usize = words.iter().map(|w| w.chars().filter(|c| !c.is_whitespace()).count()).sum();
    let (min, max) = telop_char_range(target);
    if (min..=max).contains(&total) {
        vec![]
    } else {
        vec![format!(
            "パターン{pattern_no}のキラーワードが{total}文字（目安 {min}〜{max}文字）"
        )]
    }
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for i in items {
        if !out.iter().any(|o| o == &i) {
            out.push(i);
        }
    }
    out
}

/// `#` を付け、空白を除き、重複を除いて最大5個にする
fn normalize_hashtags(raw: Vec<String>) -> Vec<String> {
    let tags = raw
        .iter()
        .flat_map(|s| s.split_whitespace())
        .map(|t| t.trim_start_matches(['#', '＃']))
        .filter(|t| !t.is_empty())
        .map(|t| format!("#{t}"))
        .collect();
    dedupe(tags).into_iter().take(MAX_HASHTAGS).collect()
}

// ─── Chapters ─────────────────────────────────────────────────────────────────

/// Gemini のチャプターを YouTube の規則（00:00始まり・昇順・10秒以上間隔・尺以内）に整える。
/// 3つ未満になった場合は字幕のタイムスタンプから機械的に作る。時刻情報の無い素材では作らない。
fn resolve_chapters(raw: &Value, video_data: &VideoData) -> Vec<Chapter> {
    if !video_data.has_captions {
        return vec![];
    }
    let chapters = normalize_chapters(parse_gemini_chapters(raw), video_data.meta.duration_seconds);
    if chapters.len() >= 3 {
        return chapters;
    }
    let fallback = build_chapters(&video_data.captions, video_data.meta.duration_seconds);
    if fallback.len() > chapters.len() { fallback } else { chapters }
}

fn parse_gemini_chapters(raw: &Value) -> Vec<Chapter> {
    let mut chapters = Vec::new();
    for item in raw.as_array().into_iter().flatten() {
        let title = str_field(item, "title");
        if title.is_empty() {
            continue;
        }
        let start_seconds = item["start_seconds"]
            .as_u64()
            .or_else(|| item["timestamp"].as_str().and_then(parse_timestamp))
            .unwrap_or(0);
        chapters.push(Chapter { timestamp: format_timestamp(start_seconds), start_seconds, title });
    }
    chapters
}

fn normalize_chapters(mut chapters: Vec<Chapter>, duration: u64) -> Vec<Chapter> {
    chapters.sort_by_key(|c| c.start_seconds);
    if duration > 0 {
        chapters.retain(|c| c.start_seconds < duration);
    }
    let mut out: Vec<Chapter> = Vec::new();
    for mut c in chapters {
        if out.is_empty() {
            c.start_seconds = 0;
        } else if c.start_seconds < out.last().unwrap().start_seconds + MIN_CHAPTER_GAP_SECS {
            continue;
        }
        c.timestamp = format_timestamp(c.start_seconds);
        out.push(c);
    }
    out
}

pub(crate) fn parse_timestamp(ts: &str) -> Option<u64> {
    let parts: Vec<&str> = ts.trim().split(':').collect();
    match parts.as_slice() {
        [h, m, s] => Some(h.parse::<u64>().ok()? * 3600 + m.parse::<u64>().ok()? * 60 + s.parse::<u64>().ok()?),
        [m, s] => Some(m.parse::<u64>().ok()? * 60 + s.parse::<u64>().ok()?),
        _ => None,
    }
}

/// 字幕から等間隔に区切った機械的なチャプター（Gemini の結果が使えない時の代替）
fn build_chapters(captions: &[CaptionSegment], total_duration: u64) -> Vec<Chapter> {
    if captions.is_empty() {
        return vec![];
    }
    let mut chapters: Vec<Chapter> = vec![Chapter {
        timestamp: "00:00".to_string(),
        start_seconds: 0,
        title: "イントロ".to_string(),
    }];

    let target_count = if total_duration < 300 { 3 } else if total_duration < 900 { 5 } else { 7 };
    let interval_secs = (total_duration / target_count as u64).max(MIN_CHAPTER_GAP_SECS);
    let mut last_chapter_start = 0u64;

    for seg in captions {
        let seg_start = seg.start_seconds as u64;
        if seg_start >= last_chapter_start + interval_secs {
            chapters.push(Chapter {
                timestamp: format_timestamp(seg_start),
                start_seconds: seg_start,
                title: derive_chapter_title(&seg.text),
            });
            last_chapter_start = seg_start;
            if chapters.len() >= target_count {
                break;
            }
        }
    }
    chapters
}

fn derive_chapter_title(text: &str) -> String {
    let trimmed = text.trim();
    let cleaned: String = trimmed.chars().take(15).collect();
    if cleaned.chars().count() < trimmed.chars().count() {
        format!("{cleaned}…")
    } else {
        cleaned
    }
}

pub fn format_timestamp(total_seconds: u64) -> String {
    let h = total_seconds / 3600;
    let m = (total_seconds % 3600) / 60;
    let s = total_seconds % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TextColorStyle, VideoMeta};

    fn video(duration: u64, captions: Vec<(f64, &str)>) -> VideoData {
        VideoData {
            meta: VideoMeta {
                title: "仮タイトル".into(),
                source_label: "字幕ファイル: test.srt".into(),
                duration_seconds: duration,
            },
            has_captions: !captions.is_empty(),
            full_transcript: captions.iter().map(|(_, t)| *t).collect::<Vec<_>>().join(" "),
            captions: captions
                .into_iter()
                .map(|(s, t)| CaptionSegment { start_seconds: s, duration_seconds: 3.0, text: t.into() })
                .collect(),
        }
    }

    fn settings() -> AppSettings {
        AppSettings {
            gemini_api_key: "k".into(),
            gemini_model: "auto".into(),
            fixed_boilerplate: "▼SNS\nhttps://example.com".into(),
            model_image_path: None,
        }
    }

    fn response(killer: &[&[&str]]) -> String {
        let patterns: Vec<Value> = killer
            .iter()
            .enumerate()
            .map(|(i, w)| {
                serde_json::json!({
                    "angle": format!("切り口{}", i + 1),
                    "killer_words": w,
                    "expression": "表情",
                    "color_scheme": "配色",
                    "decoration": "装飾",
                    "background": "背景"
                })
            })
            .collect();
        serde_json::json!({
            "tone": { "category": "解説系", "summary": "理由" },
            "thumbnail_patterns": patterns,
            "seo": {
                "title": "メインタイトル",
                "alt_titles": ["別案1", "別案2"],
                "description": "本文".repeat(100),
                "keywords": ["語1", "語2", "語1"],
                "hashtags": ["#タグ1", "タグ2", "＃タグ3 #タグ4", "#タグ1", "#タグ5", "#タグ6"],
                "chapters": [
                    { "timestamp": "00:05", "title": "導入" },
                    { "timestamp": "01:00", "title": "本題" },
                    { "timestamp": "01:04", "title": "近すぎ" },
                    { "timestamp": "09:59", "title": "尺外" },
                    { "timestamp": "02:30", "title": "まとめ" }
                ]
            }
        })
        .to_string()
    }

    #[test]
    fn test_parse_builds_three_prompts_and_normalized_seo() {
        let v = video(300, vec![(0.0, "導入"), (60.0, "本題"), (150.0, "まとめ")]);
        let raw = response(&[&["一行目", "二行目"], &["短い"], &["あ", "い", "う", "え"]]);
        let (assets, issues) = parse_gemini_response(&raw, &v, &settings(), None).unwrap();

        assert_eq!(assets.thumbnail_prompts.len(), 3);
        let ids: Vec<&str> = assets.thumbnail_prompts.iter().map(|p| p.pattern_id.as_str()).collect();
        assert_eq!(ids, ["1", "2", "3"]);
        assert_eq!(assets.thumbnail_prompts[2].killer_words, ["あ", "い", "うえ"]);
        assert!(assets.thumbnail_prompts[0].prompt.contains("1行目:「一行目」"));
        assert!(!assets.thumbnail_prompts[0].prompt.contains("【見本画像】"));

        let seo = &assets.seo;
        assert_eq!(seo.hashtags, ["#タグ1", "#タグ2", "#タグ3", "#タグ4", "#タグ5"]);
        assert_eq!(seo.keywords, ["語1", "語2"]);
        assert_eq!(seo.fixed_boilerplate, "▼SNS\nhttps://example.com");
        let ts: Vec<&str> = seo.chapters.iter().map(|c| c.timestamp.as_str()).collect();
        assert_eq!(ts, ["00:00", "01:00", "02:30"]);
        assert_eq!(assets.tone.category, "解説系");
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn test_missing_patterns_is_an_error_and_density_is_checked() {
        let v = video(300, vec![]);
        let raw = response(&[&["一"], &["二"]]);
        assert!(parse_gemini_response(&raw, &v, &settings(), None).is_err());

        let layout = ReferenceLayout { block_count: 3, block_chars: vec![8, 8, 4], total_chars: 20, ..Default::default() };
        let raw = response(&[&["短い"], &["これは十四文字ある見出しです"], &["十八文字くらいの長さ", "がある言葉"]]);
        let (_, issues) = parse_gemini_response(&raw, &v, &settings(), Some(&layout)).unwrap();
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].contains("パターン1"));
    }

    #[test]
    fn test_web_prompt_follows_reference() {
        let layout = ReferenceLayout {
            character_area: "right".into(),
            text_area: "left".into(),
            art_style: "実写写真".into(),
            composition_summary: "人物が右で文字が左".into(),
            palette: vec!["#101010".into()],
            text_styles: vec![TextColorStyle { fill: "#FFFFFF".into(), stroke: "#E00000".into(), outer: None }],
            ..Default::default()
        };
        let spec = PatternSpec {
            angle: "問いかけ".into(),
            killer_words: vec!["見出しA".into(), "見出しB".into()],
            expression: "驚いた顔".into(),
            ..Default::default()
        };
        let tone = ToneProfile { category: "解説系".into(), summary: "理由".into() };
        let p = build_web_prompt(&spec, Some(&layout), &tone);
        for needle in [
            "【見本画像】", "実写写真", "画面の右側", "画面の左側", "#101010", "塗り#FFFFFF / フチ#E00000",
            "【動画のトーン】解説系", "【このサムネの切り口】問いかけ", "1行目:「見出しA」", "2行目:「見出しB」",
            "【人物の表情・ポーズ】驚いた顔", "見本と同じ配置エリア",
        ] {
            assert!(p.contains(needle), "missing {needle}\n{p}");
        }
        assert!(!p.contains("【配色】"), "empty sections are omitted");
    }

    #[test]
    fn test_transcript_digest_has_timestamps_and_fits_budget() {
        let caps: Vec<(f64, &str)> = (0..2000).map(|i| (i as f64 * 3.0, "これは字幕のテキストです")).collect();
        let v = video(6000, caps);
        let d = transcript_digest(&v);
        assert!(d.starts_with("[00:00] "));
        assert!(d.chars().count() <= DIGEST_BUDGET_CHARS + 200);
        assert!(d.contains("[01:"));
    }

    #[test]
    fn test_chapter_fallback_from_captions() {
        let caps: Vec<(f64, &str)> = (0..100).map(|i| (i as f64 * 6.0, "話題")).collect();
        let v = video(600, caps);
        let ch = resolve_chapters(&serde_json::json!([]), &v);
        assert!(ch.len() >= 3);
        assert_eq!(ch[0].timestamp, "00:00");
    }

    #[test]
    fn test_text_source_has_no_chapters() {
        let mut v = video(0, vec![]);
        v.full_transcript = "台本の本文です。".repeat(10);
        let raw = serde_json::json!([{ "timestamp": "00:00", "title": "導入" }, { "timestamp": "01:00", "title": "本題" }]);
        assert!(resolve_chapters(&raw, &v).is_empty());

        let p = build_user_prompt(&v, None);
        assert!(p.contains("時刻情報なし"));
        assert!(p.contains("seo.chapters は空配列"));
        assert!(p.contains("動画の長さ: 不明"));
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0), "00:00");
        assert_eq!(format_timestamp(65), "01:05");
        assert_eq!(format_timestamp(3661), "01:01:01");
    }
}

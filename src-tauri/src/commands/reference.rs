/// reference.rs
///
/// 参考サムネイル（デザイン見本）の構造解析。
///
/// Gemini に見本画像を渡し、次を JSON で抽出する:
///   - テロップのブロック構成と各ブロックの文字数
///   - キャラ（人物）の配置エリア／テロップの配置エリア
///   - 画風、人物の様子、背景エフェクト、配色トーン、文字装飾、構図の要約
///   - 背景の主要色、テロップの配色ルール（塗り／フチ）
///
/// 結果は「キラーワードの文字量の目安」と「Web版Gemini向けプロンプトの見本踏襲部分」に使う。
/// AI 解析に失敗しても、画像から主要色だけはローカルで抽出して返す（処理は止めない）。

use crate::{
    commands::gemini::call_gemini_parts,
    types::{ClickSyncError, ReferenceLayout, TextColorStyle, MAX_TELOP_BLOCKS},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::DynamicImage;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const CHARACTER_AREAS: &[&str] = &["left", "right", "center", "none"];
const TEXT_AREAS: &[&str] = &["left", "right", "top", "bottom", "center"];

/// 見本にテロップが読み取れなかった場合の構成
const FALLBACK_BLOCK_CHARS: [usize; 2] = [6, 6];

fn cache() -> &'static Mutex<HashMap<String, ReferenceLayout>> {
    static CACHE: OnceLock<Mutex<HashMap<String, ReferenceLayout>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 同じファイルを差し替えた場合に再解析されるよう、更新時刻とサイズをキーに含める
fn cache_key(path: &str) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0);
    Some(format!("{path}:{mtime}:{}", meta.len()))
}

/// 見本を解析する。失敗しない（AI 不可ならローカル解析結果を返す）。
pub async fn analyze_reference(api_key: &str, path: &str) -> ReferenceLayout {
    let key = cache_key(path);
    if let Some(k) = &key {
        if let Some(hit) = cache().lock().ok().and_then(|c| c.get(k).cloned()) {
            return hit;
        }
    }

    let image = image::open(path).ok();
    let local_palette: Vec<String> = image
        .as_ref()
        .map(|img| local_palette(img).iter().map(to_hex).collect())
        .unwrap_or_default();

    let Some(img) = image else {
        eprintln!("[reference] cannot open {path}; using defaults");
        return fallback_layout(local_palette);
    };

    match analyze_with_gemini(api_key, &img).await {
        Ok(mut layout) => {
            if layout.palette.is_empty() {
                layout.palette = local_palette;
            }
            eprintln!(
                "[reference] analyzed: {} block(s) / {} chars, character={}, text={}",
                layout.block_count, layout.total_chars, layout.character_area, layout.text_area
            );
            if let (Some(k), Ok(mut c)) = (key, cache().lock()) {
                c.insert(k, layout.clone());
            }
            layout
        }
        Err(e) => {
            // 一時的な失敗の可能性があるのでキャッシュしない（次回再解析）
            eprintln!("[reference] AI analysis failed ({e}); using local palette only");
            fallback_layout(local_palette)
        }
    }
}

fn fallback_layout(palette: Vec<String>) -> ReferenceLayout {
    ReferenceLayout {
        block_count: FALLBACK_BLOCK_CHARS.len(),
        block_chars: FALLBACK_BLOCK_CHARS.to_vec(),
        total_chars: FALLBACK_BLOCK_CHARS.iter().sum(),
        block_roles: vec![],
        character_area: "none".into(),
        text_area: "left".into(),
        palette,
        analyzed_by_ai: false,
        ..Default::default()
    }
}

// ─── Gemini 解析 ───────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r##"あなたはYouTubeサムネイルのアートディレクターです。
渡されたサムネイル画像のデザイン構造を分析し、指定のJSONだけを出力してください（前置き・マークダウン不要）。

## 分析ルール
- telop_blocks: 画像内の大きな見出し文字（テロップ）を、視覚的に分かれたかたまり単位で上から順に列挙する。
  - 最大3ブロック。細かく分かれている場合は意味のまとまりで3つ以内に統合する。
  - チャンネルロゴ、再生時間、小さな注釈、透かしは含めない。
  - text は画像に書かれている文字を正確に書き起こす。role は「セリフ」「状況」「オチ」「煽り」「数字」「問いかけ」等の役割。
  - テロップが無い場合は空配列。
- character_area: 人物・キャラの主な配置。left / right / center / none のいずれか。
- text_area: テロップの主な配置。left / right / top / bottom / center のいずれか。
- 以下の説明文はすべて日本語で、画像生成AIへの指示としてそのまま使える具体的な表現にする。
- art_style: 画風（実写写真／イラスト／アニメ調／3DCG 等と、その質感）。
- subject_description: 人物・キャラの大きさ、表情、ポーズ、視線、画面に対する向き。
- background_effects: 背景とエフェクト（集中線、光、炎、ぼかし、グラデーション等。無ければ背景の様子）。
- color_tone: 全体の配色トーンと明暗のコントラスト。
- text_decoration: 文字装飾（フォントの太さ・書体の雰囲気、フチの重ね方、座布団（文字の背景帯）、吹き出し、影、傾き等）。
- composition_summary: 構図を1〜2文（主役と文字の位置関係、余白の位置）。
- palette: 背景の主要色を最大5つ、#RRGGBB。
- text_styles: テロップの配色ルールをブロックの主要なもの順に最大3つ。fill=文字の塗り、stroke=内側のフチ、outer=外側のフチ（無ければ null）。すべて #RRGGBB。

## 出力形式
{
  "telop_blocks": [{ "text": "...", "role": "..." }],
  "character_area": "left|right|center|none",
  "text_area": "left|right|top|bottom|center",
  "art_style": "...",
  "subject_description": "...",
  "background_effects": "...",
  "color_tone": "...",
  "text_decoration": "...",
  "composition_summary": "...",
  "palette": ["#RRGGBB"],
  "text_styles": [{ "fill": "#RRGGBB", "stroke": "#RRGGBB", "outer": "#RRGGBB" }]
}"##;

async fn analyze_with_gemini(
    api_key: &str,
    img: &DynamicImage,
) -> Result<ReferenceLayout, ClickSyncError> {
    let (mime, b64) = encode_for_vision(img)?;
    let parts = vec![
        json!({ "text": "このサムネイルのデザイン構造を分析してください。" }),
        json!({ "inline_data": { "mime_type": mime, "data": b64 } }),
    ];
    let raw = call_gemini_parts(api_key, SYSTEM_PROMPT, parts).await?;
    parse_layout(&raw)
}

/// トークン節約のため長辺 1024px の JPEG にして送る
fn encode_for_vision(img: &DynamicImage) -> Result<(String, String), ClickSyncError> {
    let resized = if img.width() > 1024 || img.height() > 1024 {
        img.resize(1024, 1024, image::imageops::FilterType::Triangle)
    } else {
        img.clone()
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(resized.to_rgb8())
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| ClickSyncError::ReferenceImageFailed(format!("エンコード失敗: {e}")))?;
    Ok(("image/jpeg".into(), STANDARD.encode(buf.into_inner())))
}

pub fn parse_layout(raw: &str) -> Result<ReferenceLayout, ClickSyncError> {
    let val: Value = serde_json::from_str(raw.trim())
        .map_err(|e| ClickSyncError::GeminiParseFailed(format!("見本解析JSON: {e}")))?;

    // ブロック: 空白を除いた文字数で数え、4つ目以降は3つ目へ統合
    let mut texts: Vec<String> = Vec::new();
    let mut roles: Vec<String> = Vec::new();
    for b in val["telop_blocks"].as_array().into_iter().flatten() {
        let text: String = b["text"]
            .as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if text.is_empty() {
            continue;
        }
        let role = b["role"].as_str().unwrap_or("").trim().to_string();
        if texts.len() < MAX_TELOP_BLOCKS {
            texts.push(text);
            roles.push(role);
        } else if let Some(last) = texts.last_mut() {
            last.push_str(&text);
        }
    }

    let (block_chars, block_roles) = if texts.is_empty() {
        (FALLBACK_BLOCK_CHARS.to_vec(), vec![])
    } else {
        (texts.iter().map(|t| t.chars().count()).collect::<Vec<_>>(), roles)
    };

    let pick = |key: &str, allowed: &[&str], default: &str| -> String {
        let v = val[key].as_str().unwrap_or("").trim().to_ascii_lowercase();
        if allowed.contains(&v.as_str()) { v } else { default.to_string() }
    };
    let text_str = |key: &str| val[key].as_str().unwrap_or("").trim().to_string();

    let palette: Vec<String> = val["palette"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().and_then(normalize_hex))
        .take(5)
        .collect();

    let text_styles: Vec<TextColorStyle> = val["text_styles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some(TextColorStyle {
                fill: s["fill"].as_str().and_then(normalize_hex)?,
                stroke: s["stroke"].as_str().and_then(normalize_hex)?,
                outer: s["outer"].as_str().and_then(normalize_hex),
            })
        })
        .take(3)
        .collect();

    Ok(ReferenceLayout {
        block_count: block_chars.len(),
        total_chars: block_chars.iter().sum(),
        block_chars,
        block_roles,
        character_area: pick("character_area", CHARACTER_AREAS, "none"),
        text_area: pick("text_area", TEXT_AREAS, "left"),
        art_style: text_str("art_style"),
        subject_description: text_str("subject_description"),
        background_effects: text_str("background_effects"),
        color_tone: text_str("color_tone"),
        text_decoration: text_str("text_decoration"),
        composition_summary: text_str("composition_summary"),
        palette,
        text_styles,
        analyzed_by_ai: true,
    })
}

// ─── 色ユーティリティ ──────────────────────────────────────────────────────────

/// "#RGB" / "#RRGGBB" / "RRGGBB" を "#RRGGBB"（大文字）に正規化
pub fn normalize_hex(s: &str) -> Option<String> {
    let h = s.trim().trim_start_matches('#');
    let full: String = match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        6 => h.to_string(),
        _ => return None,
    };
    if full.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("#{}", full.to_ascii_uppercase()))
    } else {
        None
    }
}

pub fn to_hex(c: &[u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// 画像の主要色（出現面積順、最大5色）。似た色はまとめる。
pub fn local_palette(img: &DynamicImage) -> Vec<[u8; 3]> {
    let small = img
        .resize_exact(48, 27, image::imageops::FilterType::Triangle)
        .to_rgb8();
    let mut buckets: HashMap<(u8, u8, u8), (u64, [u64; 3])> = HashMap::new();
    for p in small.pixels() {
        let key = (p.0[0] >> 5, p.0[1] >> 5, p.0[2] >> 5);
        let e = buckets.entry(key).or_insert((0, [0; 3]));
        e.0 += 1;
        for i in 0..3 {
            e.1[i] += p.0[i] as u64;
        }
    }
    let mut ranked: Vec<(u64, [u8; 3])> = buckets
        .into_values()
        .map(|(n, sum)| (n, [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));

    let mut out: Vec<[u8; 3]> = Vec::new();
    for (_, c) in ranked {
        let distinct = out.iter().all(|o| {
            let d: i32 = (0..3).map(|i| (o[i] as i32 - c[i] as i32).abs()).sum();
            d > 60
        });
        if distinct {
            out.push(c);
        }
        if out.len() == 5 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_layout_merges_extra_blocks_and_normalizes() {
        let raw = r##"{
          "telop_blocks": [
            {"text": "マジかよ…", "role": "セリフ"},
            {"text": "給料 全部", "role": "状況"},
            {"text": "溶けた", "role": "オチ"},
            {"text": "www", "role": "煽り"}
          ],
          "character_area": "RIGHT",
          "text_area": "diagonal",
          "background_effects": "放射状の集中線",
          "art_style": "アニメ調イラスト",
          "text_decoration": "極太ゴシック、白フチ＋黒フチの二重",
          "palette": ["#f00", "zzzzzz", "#112233"],
          "text_styles": [{"fill": "#ffffff", "stroke": "#e60012", "outer": null},
                          {"fill": "nope", "stroke": "#000000"}]
        }"##;
        let l = parse_layout(raw).unwrap();
        assert_eq!(l.block_count, 3);
        assert_eq!(l.block_chars, vec![5, 4, 6]); // 4つ目は3つ目に統合、空白は数えない
        assert_eq!(l.total_chars, 15);
        assert_eq!(l.character_area, "right");
        assert_eq!(l.text_area, "left"); // 不明値は既定へ
        assert_eq!(l.art_style, "アニメ調イラスト");
        assert!(l.text_decoration.contains("二重"));
        assert_eq!(l.palette, vec!["#FF0000", "#112233"]);
        assert_eq!(l.text_styles.len(), 1);
        assert_eq!(l.text_styles[0].stroke, "#E60012");
        assert!(l.analyzed_by_ai);
    }

    #[test]
    fn test_parse_layout_without_text_uses_fallback_structure() {
        let l = parse_layout(r#"{"telop_blocks": [], "character_area": "center"}"#).unwrap();
        assert_eq!(l.block_count, 2);
        assert_eq!(l.character_area, "center");
    }

    #[test]
    fn test_hex_helpers() {
        assert_eq!(normalize_hex("abc").as_deref(), Some("#AABBCC"));
        assert_eq!(to_hex(&[1, 2, 255]), "#0102FF");
        assert!(normalize_hex("#12345").is_none());
    }

    #[test]
    fn test_local_palette_finds_dominant_colors() {
        let img = image::RgbImage::from_fn(200, 100, |x, _| {
            if x < 140 { image::Rgb([200, 20, 20]) } else { image::Rgb([20, 20, 200]) }
        });
        let p = local_palette(&DynamicImage::ImageRgb8(img));
        assert!(p.len() >= 2);
        assert!(p[0][0] > 150 && p[0][2] < 60, "red should dominate: {:?}", p[0]);
    }
}

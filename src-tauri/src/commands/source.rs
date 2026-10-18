/// source.rs
///
/// 生成の素材（音声・字幕ファイル・台本）を読み込んで VideoData にする。
/// YouTube にはアクセスしない。音声は transcribe.rs で Gemini に文字起こしさせる。

use crate::{
    commands::transcribe::transcribe_audio,
    types::{CaptionSegment, ClickSyncError, SourceInput, VideoData, VideoMeta},
};
use regex::Regex;
use std::path::Path;

/// 字幕・台本ファイルの上限（これ以上は字幕として不自然）
const MAX_TEXT_FILE_BYTES: u64 = 20 * 1024 * 1024;
/// 台本として使う最大文字数
const MAX_SCRIPT_CHARS: usize = 60_000;

pub async fn load_source(
    api_key: &str,
    input: &SourceInput,
    topic: &str,
) -> Result<VideoData, ClickSyncError> {
    let topic = topic.trim();
    match input {
        SourceInput::Subtitle { path } => {
            let text = read_text_file(path)?;
            let captions = parse_subtitles(&text);
            if captions.is_empty() {
                return Err(ClickSyncError::SourceReadFailed(
                    "字幕を読み取れませんでした（SRT / VTT / SBV 形式のファイルを選んでください）".into(),
                ));
            }
            let duration = captions
                .last()
                .map(|c| (c.start_seconds + c.duration_seconds).ceil() as u64)
                .unwrap_or(0);
            Ok(with_captions(
                meta(topic, path, "字幕ファイル", duration),
                captions,
            ))
        }
        SourceInput::Audio { path } => {
            let (captions, duration) = transcribe_audio(api_key, Path::new(path)).await?;
            Ok(with_captions(meta(topic, path, "音声ファイル", duration), captions))
        }
        SourceInput::Text { text } => {
            let text: String = text.trim().chars().take(MAX_SCRIPT_CHARS).collect();
            if text.chars().count() < 20 {
                return Err(ClickSyncError::SourceReadFailed(
                    "台本・テキストが短すぎます（動画の内容が分かる文章を入れてください）".into(),
                ));
            }
            Ok(VideoData {
                meta: VideoMeta {
                    title: topic.to_string(),
                    source_label: "台本・テキスト".into(),
                    duration_seconds: 0,
                },
                captions: vec![],
                full_transcript: text,
                has_captions: false,
            })
        }
    }
}

fn meta(topic: &str, path: &str, kind: &str, duration_seconds: u64) -> VideoMeta {
    let p = Path::new(path);
    let file_name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let stem = p.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    VideoMeta {
        title: if topic.is_empty() { stem } else { topic.to_string() },
        source_label: format!("{kind}: {file_name}"),
        duration_seconds,
    }
}

fn with_captions(meta: VideoMeta, captions: Vec<CaptionSegment>) -> VideoData {
    let full_transcript = captions.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("　");
    VideoData { meta, captions, full_transcript, has_captions: true }
}

// ─── Text decoding ───────────────────────────────────────────────────────────

/// 字幕・台本ファイルを読み、文字コード（UTF-8 / UTF-16 / Shift_JIS）を自動判別して文字列にする
pub fn read_text_file(path: &str) -> Result<String, ClickSyncError> {
    let size = std::fs::metadata(path)
        .map_err(|e| ClickSyncError::SourceReadFailed(e.to_string()))?
        .len();
    if size > MAX_TEXT_FILE_BYTES {
        return Err(ClickSyncError::SourceReadFailed("ファイルが大きすぎます（20MBまで）".into()));
    }
    let bytes = std::fs::read(path).map_err(|e| ClickSyncError::SourceReadFailed(e.to_string()))?;
    Ok(decode_text(&bytes))
}

pub fn decode_text(bytes: &[u8]) -> String {
    if let Some((encoding, bom_len)) = encoding_rs::Encoding::for_bom(bytes) {
        return encoding.decode_without_bom_handling(&bytes[bom_len..]).0.into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::SHIFT_JIS.decode(bytes).0.into_owned(),
    }
}

// ─── Subtitle parsing ────────────────────────────────────────────────────────

/// SRT / WebVTT / SBV を時刻付きの字幕に変換する。
/// 形式ごとの違い（区切り文字・番号行・スタイル）は吸収し、連続する重複行は1つにまとめる。
pub fn parse_subtitles(text: &str) -> Vec<CaptionSegment> {
    let arrow = Regex::new(r"^\s*([\d:.,]+)\s*-->\s*([\d:.,]+)").unwrap();
    let sbv = Regex::new(r"^\s*(\d+:\d{2}:\d{2}\.\d{1,3}),(\d+:\d{2}:\d{2}\.\d{1,3})\s*$").unwrap();
    let tag = Regex::new(r"<[^>]*>|\{\\[^}]*\}").unwrap();

    let mut out: Vec<CaptionSegment> = Vec::new();
    let mut current: Option<(f64, f64, Vec<String>)> = None;

    let flush = |cur: &mut Option<(f64, f64, Vec<String>)>, out: &mut Vec<CaptionSegment>| {
        if let Some((start, end, lines)) = cur.take() {
            let text = lines.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() || out.last().is_some_and(|prev| prev.text == text) {
                return;
            }
            out.push(CaptionSegment {
                start_seconds: start,
                duration_seconds: (end - start).max(0.0),
                text,
            });
        }
    };

    for line in text.lines() {
        let timing = arrow
            .captures(line)
            .or_else(|| sbv.captures(line))
            .and_then(|c| Some((parse_clock(&c[1])?, parse_clock(&c[2])?)));
        if let Some((start, end)) = timing {
            flush(&mut current, &mut out);
            current = Some((start, end, Vec::new()));
            continue;
        }
        if line.trim().is_empty() {
            flush(&mut current, &mut out);
            continue;
        }
        if let Some((_, _, lines)) = current.as_mut() {
            let cleaned = tag.replace_all(line, "").replace("&nbsp;", " ").replace("&amp;", "&");
            let cleaned = cleaned.trim();
            if !cleaned.is_empty() {
                lines.push(cleaned.to_string());
            }
        }
    }
    flush(&mut current, &mut out);
    out.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
    out
}

/// "01:02:03,456" / "02:03.456" / "1:02:03.4" → 秒
fn parse_clock(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_srt() {
        let srt = "1\n00:00:01,000 --> 00:00:03,500\nこんにちは\n二行目\n\n2\n00:01:05,000 --> 00:01:07,000\n<i>本題</i>です\n";
        let caps = parse_subtitles(srt);
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0].text, "こんにちは 二行目");
        assert!((caps[0].duration_seconds - 2.5).abs() < 1e-6);
        assert_eq!(caps[1].start_seconds, 65.0);
        assert_eq!(caps[1].text, "本題です");
    }

    #[test]
    fn test_parse_vtt_with_settings_and_rolling_duplicates() {
        let vtt = "WEBVTT\nKind: captions\n\nNOTE メモ\n\n00:00.500 --> 00:02.000 align:start position:0%\nはじめに\n\n00:02.000 --> 00:04.000\nはじめに\n\n01:00:00.000 --> 01:00:02.000\n終わり\n";
        let caps = parse_subtitles(vtt);
        let texts: Vec<&str> = caps.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["はじめに", "終わり"]);
        assert_eq!(caps[1].start_seconds, 3600.0);
    }

    #[test]
    fn test_parse_sbv() {
        let sbv = "0:00:01.000,0:00:03.000\n一つ目\n\n0:00:10.500,0:00:12.000\n二つ目\n";
        let caps = parse_subtitles(sbv);
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[1].start_seconds, 10.5);
    }

    #[test]
    fn test_decode_shift_jis_and_bom() {
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("字幕テスト");
        assert_eq!(decode_text(&sjis), "字幕テスト");
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice("台本".as_bytes());
        assert_eq!(decode_text(&bom), "台本");
    }

    #[test]
    fn test_plain_text_is_not_subtitles() {
        assert!(parse_subtitles("これはただの台本です。\n時刻はありません。").is_empty());
    }
}

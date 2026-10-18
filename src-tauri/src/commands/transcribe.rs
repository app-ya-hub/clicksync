/// transcribe.rs
///
/// 音声ファイルを Gemini Files API にアップロードし、時刻付きの区間要約として文字起こしする。
///   1. resumable upload（ファイルはメモリに載せずストリーミング送信）
///   2. 処理完了（ACTIVE）を待つ
///   3. generateContent で区間ごとの要約を JSON で受け取る
///   4. アップロードしたファイルを削除する（成功・失敗にかかわらず）
///
/// 一字一句の書き起こしではなく区間要約にしているのは、長い音声でも出力上限で途切れず、
/// 時刻のずれも区間単位に収まるようにするため。

use crate::{
    commands::{
        gemini::{call_gemini_with, extract_json, CallOptions},
        generate::parse_timestamp,
    },
    types::{CaptionSegment, ClickSyncError},
};
use reqwest::{Body, Client};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tokio_util::io::ReaderStream;

const API_BASE: &str = "https://generativelanguage.googleapis.com";
/// Files API の上限
const MAX_AUDIO_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const PROCESSING_TIMEOUT: Duration = Duration::from_secs(600);
const MAX_SEGMENTS: usize = 90;

/// 対応する音声形式（拡張子 → MIME）
pub const AUDIO_EXTENSIONS: &[(&str, &str)] = &[
    ("mp3", "audio/mp3"),
    ("wav", "audio/wav"),
    ("m4a", "audio/mp4"),
    ("aac", "audio/aac"),
    ("ogg", "audio/ogg"),
    ("flac", "audio/flac"),
    ("aiff", "audio/aiff"),
    ("aif", "audio/aiff"),
];

const SYSTEM_PROMPT: &str = r#"あなたは動画編集のアシスタントです。添付の音声を最後まで聞き、内容を時系列の区間に分けて、指定のJSONだけを返してください。

- duration_seconds: 音声全体の長さ（秒・整数）
- segments: 区間の配列
  - 話題の切れ目を優先し、30〜60秒程度ごとに区切る（最大90区間）
  - start: その話が実際に始まる時刻（"mm:ss" または "h:mm:ss"）。推測で埋めない
  - text: その区間で話している内容を、固有名詞・数字・印象的な言い回しを残して80文字以内に要約
- 無音・BGMだけの区間は含めない
- 内容を聞き取れない場合は segments を空配列にする

{ "duration_seconds": 0, "segments": [{ "start": "00:00", "text": "..." }] }"#;

/// 音声を文字起こしし、（区間ごとの字幕, 音声の長さ秒）を返す
pub async fn transcribe_audio(
    api_key: &str,
    path: &Path,
) -> Result<(Vec<CaptionSegment>, u64), ClickSyncError> {
    if api_key.is_empty() {
        return Err(ClickSyncError::ApiKeyMissing);
    }
    let mime = audio_mime(path).ok_or_else(|| {
        ClickSyncError::SourceReadFailed(format!(
            "対応していない形式です（{}）",
            AUDIO_EXTENSIONS.iter().map(|(e, _)| *e).collect::<Vec<_>>().join(" / ")
        ))
    })?;
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|e| ClickSyncError::SourceReadFailed(e.to_string()))?
        .len();
    if size == 0 || size > MAX_AUDIO_BYTES {
        return Err(ClickSyncError::SourceReadFailed("音声ファイルは2GBまでです".into()));
    }

    let client = Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;

    let file = upload(&client, api_key, path, mime, size).await?;
    let name = file_name(&file)
        .ok_or_else(|| ClickSyncError::TranscriptionFailed("アップロード結果が不正です".into()))?;
    let result = async {
        let file = wait_until_active(&client, api_key, &name, file).await?;
        request_transcript(api_key, &file, mime).await
    }
    .await;
    delete_file(&client, api_key, &name).await;
    result
}

pub fn audio_mime(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    AUDIO_EXTENSIONS.iter().find(|(e, _)| *e == ext).map(|(_, m)| *m)
}

// ─── Files API ────────────────────────────────────────────────────────────────

async fn upload(
    client: &Client,
    api_key: &str,
    path: &Path,
    mime: &str,
    size: u64,
) -> Result<Value, ClickSyncError> {
    let start = client
        .post(format!("{API_BASE}/upload/v1beta/files?key={api_key}"))
        .header("X-Goog-Upload-Protocol", "resumable")
        .header("X-Goog-Upload-Command", "start")
        .header("X-Goog-Upload-Header-Content-Length", size.to_string())
        .header("X-Goog-Upload-Header-Content-Type", mime)
        .json(&json!({ "file": { "display_name": "clicksync-audio" } }))
        .send()
        .await
        .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;
    let start = ensure_ok(start).await?;
    let upload_url = start
        .headers()
        .get("x-goog-upload-url")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ClickSyncError::TranscriptionFailed("アップロード先を取得できませんでした".into()))?
        .to_string();

    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| ClickSyncError::SourceReadFailed(e.to_string()))?;
    let resp = client
        .post(upload_url)
        .header("Content-Length", size.to_string())
        .header("X-Goog-Upload-Offset", "0")
        .header("X-Goog-Upload-Command", "upload, finalize")
        .body(Body::wrap_stream(ReaderStream::new(file)))
        .send()
        .await
        .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;
    let data: Value = ensure_ok(resp)
        .await?
        .json()
        .await
        .map_err(|e| ClickSyncError::TranscriptionFailed(e.to_string()))?;
    Ok(data["file"].clone())
}

async fn wait_until_active(
    client: &Client,
    api_key: &str,
    name: &str,
    mut file: Value,
) -> Result<Value, ClickSyncError> {
    let started = std::time::Instant::now();
    loop {
        match file["state"].as_str() {
            Some("ACTIVE") => return Ok(file),
            Some("FAILED") => {
                return Err(ClickSyncError::TranscriptionFailed(
                    "Gemini 側で音声を処理できませんでした（mp3 など別の形式で書き出して試してください）".into(),
                ))
            }
            _ => {}
        }
        if started.elapsed() > PROCESSING_TIMEOUT {
            return Err(ClickSyncError::TranscriptionFailed("音声の処理がタイムアウトしました".into()));
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
        let resp = client
            .get(format!("{API_BASE}/v1beta/{name}?key={api_key}"))
            .send()
            .await
            .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;
        file = ensure_ok(resp)
            .await?
            .json()
            .await
            .map_err(|e| ClickSyncError::TranscriptionFailed(e.to_string()))?;
    }
}

async fn delete_file(client: &Client, api_key: &str, name: &str) {
    let result = client
        .delete(format!("{API_BASE}/v1beta/{name}?key={api_key}"))
        .send()
        .await;
    if let Err(e) = result {
        eprintln!("[transcribe] failed to delete uploaded file: {e}");
    }
}

fn file_name(file: &Value) -> Option<String> {
    file["name"].as_str().filter(|n| n.starts_with("files/")).map(str::to_string)
}

async fn ensure_ok(resp: reqwest::Response) -> Result<reqwest::Response, ClickSyncError> {
    if resp.status().is_success() {
        return Ok(resp);
    }
    let status = resp.status().as_u16();
    if matches!(status, 429 | 500 | 502 | 503 | 504) {
        return Err(ClickSyncError::GeminiBusy { status });
    }
    let body = resp.text().await.unwrap_or_default();
    let message = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.pointer("/error/message").and_then(|m| m.as_str()).map(str::to_string))
        .unwrap_or_else(|| body.chars().take(200).collect());
    Err(ClickSyncError::GeminiApiError { status, message })
}

// ─── Transcription ────────────────────────────────────────────────────────────

async fn request_transcript(
    api_key: &str,
    file: &Value,
    mime: &str,
) -> Result<(Vec<CaptionSegment>, u64), ClickSyncError> {
    let uri = file["uri"]
        .as_str()
        .ok_or_else(|| ClickSyncError::TranscriptionFailed("アップロード結果が不正です".into()))?;
    let parts = vec![
        json!({ "file_data": { "mime_type": file["mimeType"].as_str().unwrap_or(mime), "file_uri": uri } }),
        json!({ "text": "この音声を区間ごとに要約してください。" }),
    ];
    let options = CallOptions { max_output_tokens: 32_768, timeout: Duration::from_secs(900) };

    let mut attempt = 0;
    loop {
        attempt += 1;
        let raw = call_gemini_with(api_key, SYSTEM_PROMPT, parts.clone(), options).await?;
        match parse_transcript(&raw) {
            Err(ClickSyncError::TranscriptionFailed(reason)) if attempt < 2 => {
                eprintln!("[transcribe] attempt {attempt} unusable ({reason}); reply head: {}", head(&raw, 500));
            }
            Err(ClickSyncError::TranscriptionFailed(reason)) => {
                eprintln!("[transcribe] attempt {attempt} unusable ({reason}); reply head: {}", head(&raw, 500));
                return Err(ClickSyncError::TranscriptionFailed(format!(
                    "{reason}（AIの返答の先頭: 「{}」）",
                    head(&raw, 100)
                )));
            }
            other => return other,
        }
    }
}

fn head(s: &str, max_chars: usize) -> String {
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = flat.chars().take(max_chars).collect();
    if flat.chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn parse_transcript(raw: &str) -> Result<(Vec<CaptionSegment>, u64), ClickSyncError> {
    let val: Value = serde_json::from_str(extract_json(raw))
        .map_err(|e| ClickSyncError::TranscriptionFailed(format!("応答を解析できませんでした: {e}")))?;
    let reported = val["duration_seconds"].as_u64().unwrap_or(0);

    let mut points: Vec<(u64, String)> = val["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let start = s["start"].as_str().and_then(parse_timestamp)?;
            let text = s["text"].as_str()?.trim().to_string();
            (!text.is_empty()).then_some((start, text))
        })
        .collect();
    points.sort_by_key(|(s, _)| *s);
    points.dedup_by_key(|(s, _)| *s);
    if reported > 0 {
        points.retain(|(s, _)| *s <= reported);
    }
    points.truncate(MAX_SEGMENTS);
    if points.is_empty() {
        return Err(ClickSyncError::TranscriptionFailed(
            "音声から内容を聞き取れませんでした".into(),
        ));
    }

    let last_start = points.last().map(|(s, _)| *s).unwrap_or(0);
    let duration = if reported > last_start { reported } else { last_start + 30 };
    let caps = points
        .iter()
        .enumerate()
        .map(|(i, (start, text))| {
            let end = points.get(i + 1).map(|(s, _)| *s).unwrap_or(duration);
            CaptionSegment {
                start_seconds: *start as f64,
                duration_seconds: end.saturating_sub(*start) as f64,
                text: text.clone(),
            }
        })
        .collect();
    Ok((caps, duration))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_transcript_sorts_and_bounds() {
        let raw = r#"{"duration_seconds": 300, "segments": [
            {"start": "01:00", "text": "本題"},
            {"start": "00:00", "text": "導入"},
            {"start": "01:00", "text": "重複"},
            {"start": "09:00", "text": "尺外"},
            {"start": "02:30", "text": ""}
        ]}"#;
        let (caps, duration) = parse_transcript(raw).unwrap();
        assert_eq!(duration, 300);
        let texts: Vec<&str> = caps.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["導入", "本題"]);
        assert_eq!(caps[0].duration_seconds, 60.0);
        assert_eq!(caps[1].duration_seconds, 240.0);
    }

    #[test]
    fn test_parse_transcript_tolerates_wrapped_reply() {
        let raw = "以下が結果です。\n```JSON\n{\"duration_seconds\": 90, \"segments\": [{\"start\": \"00:00\", \"text\": \"導入\"}]}\n```";
        let (caps, duration) = parse_transcript(raw).unwrap();
        assert_eq!(duration, 90);
        assert_eq!(caps[0].text, "導入");
    }

    #[test]
    fn test_head_flattens_and_truncates() {
        assert_eq!(head("申し訳\n ありません", 100), "申し訳 ありません");
        assert_eq!(head("あいうえお", 3), "あいう…");
    }

    #[test]
    fn test_empty_transcript_is_error() {
        assert!(parse_transcript(r#"{"duration_seconds": 10, "segments": []}"#).is_err());
    }

    #[test]
    fn test_audio_mime() {
        assert_eq!(audio_mime(Path::new("/a/Talk.MP3")), Some("audio/mp3"));
        assert_eq!(audio_mime(Path::new("/a/movie.mp4")), None);
    }
}

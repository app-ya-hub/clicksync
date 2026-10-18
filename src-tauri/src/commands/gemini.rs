/// gemini.rs
///
/// Low-level Gemini API client: text generateContent + models.list.
///
/// モデルIDは一切ハードコードしない。models.list の結果から Flash 系を
/// バージョン降順の配列として組み立て、インデックスで選ぶ。
/// 既定は「最新の1世代前」で、混雑(429/5xx)や未提供(404)なら次に古い
/// インデックスへ自動フォールバックする。

use crate::types::{ClickSyncError, GeminiModelCatalog, GeminiModelInfo};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const CATALOG_TTL: Duration = Duration::from_secs(10 * 60);

/// 最新は混雑しやすいため、既定はこの数だけ古い世代から始める。
const PREFERRED_GENERATION_OFFSET: usize = 1;

/// generateContent に渡す thinking レベル。非対応モデルでは自動で外す。
const THINKING_LEVEL: &str = "LOW";
/// 全モデルが混雑(429/5xx)だったときに、全体をやり直すまでの待ち時間
const BUSY_RETRY_WAITS: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(15)];

// ─── Caches ──────────────────────────────────────────────────────────────────

struct CachedModels {
    fetched_at: Instant,
    models: Vec<RawModel>,
}

fn raw_cache() -> &'static RwLock<Option<CachedModels>> {
    static CACHE: OnceLock<RwLock<Option<CachedModels>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(None))
}

/// 直近のリクエストが成功したモデルID（画面の「実行モデル」表示用）。
fn active_model() -> &'static RwLock<Option<String>> {
    static ACTIVE: OnceLock<RwLock<Option<String>>> = OnceLock::new();
    ACTIVE.get_or_init(|| RwLock::new(None))
}

pub async fn last_successful_model() -> Option<String> {
    active_model().read().await.clone()
}

async fn remember_successful_model(id: &str) {
    *active_model().write().await = Some(id.to_string());
}

// ─── Model metadata ──────────────────────────────────────────────────────────

#[derive(Clone)]
struct RawModel {
    id: String,
    display_name: String,
    supports_generate_content: bool,
}

pub fn normalize_model_id(raw: &str) -> String {
    raw.trim().trim_start_matches("models/").trim().to_string()
}

/// `gemini-<major>[.<minor>]-...` からバージョンを取り出す。特定の番号には依存しない。
fn parse_model_version(id: &str) -> Option<(u32, u32)> {
    let rest = id.strip_prefix("gemini-")?;
    let token = rest.split('-').next()?;
    if !token.chars().next()?.is_ascii_digit() {
        return None;
    }
    let mut parts = token.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

fn is_preview_or_experimental(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("preview") || l.contains("exp") || l.contains("-latest")
}

/// テキスト生成に使う Flash 系。画像専用・Lite は除外する。
fn is_text_flash_model(id: &str) -> bool {
    let l = id.to_ascii_lowercase();
    l.contains("flash") && !l.contains("image") && !l.contains("lite")
}

fn is_text_generate_model(id: &str, supports_generate_content: bool) -> bool {
    let lower = id.to_ascii_lowercase();
    if lower.contains("embed")
        || lower.contains("image")
        || lower.contains("imagen")
        || lower.contains("veo")
        || lower.contains("tts")
        || lower.contains("aqa")
        || lower.contains("robotics")
    {
        return false;
    }
    supports_generate_content && lower.contains("gemini")
}

fn cmp_model_ids_newest_first(a: &str, b: &str) -> std::cmp::Ordering {
    let va = parse_model_version(a).unwrap_or((0, 0));
    let vb = parse_model_version(b).unwrap_or((0, 0));
    vb.0
        .cmp(&va.0)
        .then(vb.1.cmp(&va.1))
        .then(is_preview_or_experimental(a).cmp(&is_preview_or_experimental(b)))
        .then(a.len().cmp(&b.len()))
        .then(a.cmp(b))
}

/// 同一バージョンでは安定版・短いIDを代表に選ぶ。
fn sort_candidates(ids: &mut Vec<String>) {
    ids.sort_by(|a, b| cmp_model_ids_newest_first(a, b));
}

/// バージョン降順の候補配列を作る。1バージョン=1エントリにして
/// インデックスが「世代」を意味するようにする。
pub fn build_candidates<I, S>(ids: I, keep: fn(&str) -> bool) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut matching: Vec<String> = ids
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .filter(|id| keep(id))
        .collect();
    matching.dedup();
    sort_candidates(&mut matching);

    let mut seen_versions: Vec<(u32, u32)> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    for id in matching {
        match parse_model_version(&id) {
            Some(ver) => {
                if !seen_versions.contains(&ver) {
                    seen_versions.push(ver);
                    out.push(id);
                }
            }
            // バージョンが読めないIDも末尾の候補として残す
            None => out.push(id),
        }
    }
    out
}

/// 既定の開始インデックス（最新の1世代前。候補が1つなら0）。
pub fn preferred_index(len: usize) -> usize {
    if len > PREFERRED_GENERATION_OFFSET {
        PREFERRED_GENERATION_OFFSET
    } else {
        0
    }
}

/// 開始インデックスから古い方へ、その後に新しい方へ戻る試行順。
fn fallback_order(len: usize, start: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (start..len).collect();
    order.extend((0..start).rev());
    order
}

// ─── models.list ─────────────────────────────────────────────────────────────

async fn fetch_raw_models(api_key: &str) -> Result<Vec<RawModel>, ClickSyncError> {
    if api_key.is_empty() {
        return Err(ClickSyncError::ApiKeyMissing);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;

    let mut models: Vec<RawModel> = Vec::new();
    let mut page_token = String::new();

    loop {
        let mut url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}&pageSize=100"
        );
        if !page_token.is_empty() {
            url.push_str(&format!("&pageToken={page_token}"));
        }

        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;

        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(ClickSyncError::GeminiApiError {
                status,
                message: extract_gemini_error_message(&err_body),
            });
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| ClickSyncError::GeminiParseFailed(e.to_string()))?;

        if let Some(arr) = data.get("models").and_then(|v| v.as_array()) {
            for m in arr {
                let id = normalize_model_id(m.get("name").and_then(|v| v.as_str()).unwrap_or(""));
                if id.is_empty() {
                    continue;
                }
                let display_name = m
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string();
                let supports_generate_content = m
                    .get("supportedGenerationMethods")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .any(|s| s.eq_ignore_ascii_case("generateContent"))
                    })
                    .unwrap_or(true);
                models.push(RawModel {
                    id,
                    display_name,
                    supports_generate_content,
                });
            }
        }

        page_token = data
            .get("nextPageToken")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if page_token.is_empty() {
            break;
        }
    }

    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);
    Ok(models)
}

async fn cached_raw_models(api_key: &str, force: bool) -> Result<Vec<RawModel>, ClickSyncError> {
    if !force {
        let guard = raw_cache().read().await;
        if let Some(cached) = guard.as_ref() {
            if cached.fetched_at.elapsed() < CATALOG_TTL {
                return Ok(cached.models.clone());
            }
        }
    }

    let models = fetch_raw_models(api_key).await?;
    *raw_cache().write().await = Some(CachedModels {
        fetched_at: Instant::now(),
        models: models.clone(),
    });
    Ok(models)
}

/// UI 表示用の一覧（新しい順）+ 自動選定されたインデックス。
pub async fn get_model_catalog(
    api_key: &str,
    force: bool,
) -> Result<GeminiModelCatalog, ClickSyncError> {
    let raw = cached_raw_models(api_key, force).await?;

    let text_models: Vec<&RawModel> = raw
        .iter()
        .filter(|m| is_text_generate_model(&m.id, m.supports_generate_content))
        .collect();

    let candidates = build_candidates(text_models.iter().map(|m| m.id.as_str()), is_text_flash_model);
    let auto_selected_index = preferred_index(candidates.len());
    let auto_selected_id = candidates
        .get(auto_selected_index)
        .cloned()
        .or_else(|| text_models.first().map(|m| m.id.clone()))
        .unwrap_or_default();
    let auto_selected_display_name = text_models
        .iter()
        .find(|m| m.id == auto_selected_id)
        .map(|m| m.display_name.clone())
        .unwrap_or_else(|| auto_selected_id.clone());

    let mut models: Vec<GeminiModelInfo> = text_models
        .into_iter()
        .map(|m| GeminiModelInfo {
            recommended_default: m.id == auto_selected_id,
            id: m.id.clone(),
            display_name: m.display_name.clone(),
        })
        .collect();
    models.sort_by(|a, b| cmp_model_ids_newest_first(&a.id, &b.id));

    Ok(GeminiModelCatalog {
        models,
        candidates,
        auto_selected_index,
        auto_selected_id,
        auto_selected_display_name,
    })
}

// ─── generateContent ─────────────────────────────────────────────────────────

fn safety_settings() -> Value {
    json!([
        { "category": "HARM_CATEGORY_HARASSMENT",        "threshold": "BLOCK_NONE" },
        { "category": "HARM_CATEGORY_HATE_SPEECH",       "threshold": "BLOCK_NONE" },
        { "category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "threshold": "BLOCK_NONE" },
        { "category": "HARM_CATEGORY_DANGEROUS_CONTENT", "threshold": "BLOCK_NONE" }
    ])
}

/// 1回の generateContent 呼び出しの上限
#[derive(Debug, Clone, Copy)]
pub struct CallOptions {
    pub max_output_tokens: u32,
    pub timeout: Duration,
}

impl Default for CallOptions {
    fn default() -> Self {
        Self { max_output_tokens: 8192, timeout: Duration::from_secs(120) }
    }
}

fn generation_config(with_thinking: bool, max_output_tokens: u32) -> Value {
    let mut cfg = json!({
        "temperature": 0.85,
        "topP": 0.95,
        "topK": 40,
        "maxOutputTokens": max_output_tokens,
        "responseMimeType": "application/json"
    });
    if with_thinking {
        cfg["thinkingConfig"] = json!({ "thinkingLevel": THINKING_LEVEL });
    }
    cfg
}

/// 混雑・一時障害。別モデルへ切り替える価値がある。
fn is_capacity_error(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// そのモデルが使えない。次の候補へ。
fn is_model_unavailable(status: u16, message: &str) -> bool {
    if status == 404 {
        return true;
    }
    if status != 400 {
        return false;
    }
    let m = message.to_ascii_lowercase();
    m.contains("not found") || m.contains("not supported") || m.contains("no longer available")
}

fn thinking_rejected(status: u16, message: &str) -> bool {
    status == 400 && message.to_ascii_lowercase().contains("thinking")
}

pub async fn call_gemini(
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, ClickSyncError> {
    call_gemini_parts(api_key, system_prompt, vec![json!({ "text": user_prompt })]).await
}

/// テキスト＋画像などのマルチモーダル parts を渡す版（JSON 応答を返す）
pub async fn call_gemini_parts(
    api_key: &str,
    system_prompt: &str,
    parts: Vec<Value>,
) -> Result<String, ClickSyncError> {
    call_gemini_with(api_key, system_prompt, parts, CallOptions::default()).await
}

pub async fn call_gemini_with(
    api_key: &str,
    system_prompt: &str,
    parts: Vec<Value>,
    options: CallOptions,
) -> Result<String, ClickSyncError> {
    if api_key.is_empty() {
        return Err(ClickSyncError::ApiKeyMissing);
    }

    let catalog = get_model_catalog(api_key, false).await?;
    if catalog.candidates.is_empty() {
        return Err(ClickSyncError::NoModelsAvailable);
    }

    let client = Client::builder()
        .timeout(options.timeout)
        .build()
        .map_err(|e| ClickSyncError::NetworkError(e.to_string()))?;

    let order = fallback_order(catalog.candidates.len(), catalog.auto_selected_index);
    let mut last_err: Option<ClickSyncError> = None;

    for round in 0..=BUSY_RETRY_WAITS.len() {
        if round > 0 {
            let wait = BUSY_RETRY_WAITS[round - 1];
            eprintln!("[gemini] all models busy, retrying in {}s", wait.as_secs());
            tokio::time::sleep(wait).await;
        }
        let mut busy_status: Option<u16> = None;
        let mut all_busy = true;

        for &index in &order {
            let model = &catalog.candidates[index];
            match try_generate(&client, api_key, model, system_prompt, &parts, options.max_output_tokens).await {
                Ok(text) => {
                    remember_successful_model(model).await;
                    return Ok(text);
                }
                Err(Attempt::NextModel(e)) => {
                    eprintln!("[gemini] {model} unavailable/busy, falling back: {e}");
                    match &e {
                        ClickSyncError::GeminiApiError { status, .. } if is_capacity_error(*status) => {
                            busy_status.get_or_insert(*status);
                        }
                        ClickSyncError::NetworkError(_) => {}
                        _ => all_busy = false,
                    }
                    last_err = Some(e);
                }
                Err(Attempt::Fatal(e)) => return Err(e),
            }
        }

        if !all_busy {
            break;
        }
        if let Some(status) = busy_status {
            last_err = Some(ClickSyncError::GeminiBusy { status });
        }
    }

    Err(last_err.unwrap_or(ClickSyncError::NoModelsAvailable))
}

enum Attempt {
    /// 次のモデル候補を試す
    NextModel(ClickSyncError),
    /// モデルを変えても解決しない
    Fatal(ClickSyncError),
}

async fn try_generate(
    client: &Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    parts: &[Value],
    max_output_tokens: u32,
) -> Result<String, Attempt> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );
    let mut with_thinking = true;

    loop {
        let body = json!({
            "systemInstruction": { "parts": [{ "text": system_prompt }] },
            "contents": [{
                "role": "user",
                "parts": parts
            }],
            "generationConfig": generation_config(with_thinking, max_output_tokens),
            "safetySettings": safety_settings()
        });

        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| Attempt::NextModel(ClickSyncError::NetworkError(e.to_string())))?;

        let status = resp.status().as_u16();

        if !resp.status().is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            let message = extract_gemini_error_message(&err_body);

            if with_thinking && thinking_rejected(status, &message) {
                with_thinking = false;
                continue;
            }

            let err = ClickSyncError::GeminiApiError { status, message };
            return Err(if is_capacity_error(status)
                || is_model_unavailable(status, &err.to_string())
            {
                Attempt::NextModel(err)
            } else {
                Attempt::Fatal(err)
            });
        }

        let data: Value = resp
            .json()
            .await
            .map_err(|e| Attempt::Fatal(ClickSyncError::GeminiParseFailed(e.to_string())))?;

        let text = response_text(&data);
        if text.trim().is_empty() {
            return Err(Attempt::NextModel(ClickSyncError::GeminiParseFailed(
                "レスポンスにテキストが含まれていません".into(),
            )));
        }
        return Ok(extract_json(&text).to_string());
    }
}

/// 応答の全パートのテキストを連結する（長い応答は複数パートに分かれる。思考パートは除く）
fn response_text(data: &Value) -> String {
    data.pointer("/candidates/0/content/parts")
        .and_then(|p| p.as_array())
        .into_iter()
        .flatten()
        .filter(|part| !part["thought"].as_bool().unwrap_or(false))
        .filter_map(|part| part["text"].as_str())
        .collect()
}

/// 前置きの文章や ```json などの囲みが付いていても、最初の `{`/`[` から対応する最後の `}`/`]` までを取り出す。
/// JSON が見つからなければ前後の空白を除いた全文を返す（呼び出し側で解析エラーになる）。
pub fn extract_json(s: &str) -> &str {
    let s = s.trim();
    let Some(start) = s.find(['{', '[']) else {
        return s;
    };
    let close = if s.as_bytes()[start] == b'{' { '}' } else { ']' };
    match s.rfind(close) {
        Some(end) if end > start => &s[start..=end],
        _ => &s[start..],
    }
}

fn extract_gemini_error_message(body: &str) -> String {
    if let Ok(val) = serde_json::from_str::<Value>(body) {
        if let Some(msg) = val.pointer("/error/message").and_then(|v| v.as_str()) {
            return msg.to_string();
        }
    }
    body.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidates_are_newest_first_one_per_version() {
        let ids = [
            "gemini-7.0-flash",
            "gemini-6.4-flash",
            "gemini-6.4-flash-preview",
            "gemini-6.1-flash",
            "gemini-6.4-pro",
        ];
        assert_eq!(
            build_candidates(ids, is_text_flash_model),
            vec!["gemini-7.0-flash", "gemini-6.4-flash", "gemini-6.1-flash"]
        );
    }

    #[test]
    fn test_preferred_index_is_one_generation_behind() {
        assert_eq!(preferred_index(3), 1);
        assert_eq!(preferred_index(2), 1);
        assert_eq!(preferred_index(1), 0);
        assert_eq!(preferred_index(0), 0);
    }

    #[test]
    fn test_fallback_order_goes_older_then_newer() {
        assert_eq!(fallback_order(4, 1), vec![1, 2, 3, 0]);
        assert_eq!(fallback_order(1, 0), vec![0]);
    }

    #[test]
    fn test_capacity_errors_trigger_fallback() {
        assert!(is_capacity_error(429));
        assert!(is_capacity_error(503));
        assert!(!is_capacity_error(400));
        assert!(!is_capacity_error(401));
    }

    #[test]
    fn test_model_unavailable_detection() {
        assert!(is_model_unavailable(404, "not found"));
        assert!(is_model_unavailable(400, "models/x is not found"));
        assert!(!is_model_unavailable(400, "invalid argument"));
    }

    #[test]
    fn test_extract_json_tolerates_wrappers() {
        assert_eq!(extract_json("```JSON\n{\"a\": 1}\n```"), "{\"a\": 1}");
        assert_eq!(extract_json("以下が結果です。\n{\"a\": {\"b\": 2}}\nご確認ください。"), "{\"a\": {\"b\": 2}}");
        assert_eq!(extract_json("  [1, 2]  "), "[1, 2]");
        assert_eq!(extract_json("申し訳ありません"), "申し訳ありません");
    }

    #[test]
    fn test_response_text_joins_parts_and_skips_thoughts() {
        let data = json!({ "candidates": [{ "content": { "parts": [
            { "text": "考え中", "thought": true },
            { "text": "{\"a\": " },
            { "text": "1}" }
        ]}}]});
        assert_eq!(response_text(&data), "{\"a\": 1}");
    }

    #[test]
    fn test_normalize_model_id() {
        assert_eq!(normalize_model_id("models/gemini-6.4-flash"), "gemini-6.4-flash");
    }
}

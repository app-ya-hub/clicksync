pub mod gemini;
pub mod generate;
pub mod reference;
pub mod source;
pub mod transcribe;

use crate::{
    store::settings::{has_api_key, load_settings, save_settings},
    types::{
        AppSettings, ClickSyncError, GeminiModelCatalog, GeminiModelInfo, GeneratedAssets, SourceInput,
        VideoData,
    },
};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(serde::Serialize)]
pub struct PickedFile {
    path: String,
    name: String,
    size: u64,
}

/// IPC: 素材ファイルをネイティブのダイアログで選ぶ（kind: audio / subtitle / text）。
/// 大きな音声もコピーせずに済むよう、中身ではなく絶対パスを返す。
#[tauri::command]
pub async fn cmd_pick_source_file(app: AppHandle, kind: String) -> Result<Option<PickedFile>, ClickSyncError> {
    let audio: Vec<&str> = transcribe::AUDIO_EXTENSIONS.iter().map(|(ext, _)| *ext).collect();
    let (label, exts): (&str, Vec<&str>) = match kind.as_str() {
        "audio" => ("音声ファイル", audio),
        "subtitle" => ("字幕ファイル", vec!["srt", "vtt", "sbv"]),
        "text" => ("テキスト", vec!["txt", "md"]),
        other => return Err(ClickSyncError::SourceReadFailed(format!("不明な種類: {other}"))),
    };

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().add_filter(label, &exts).pick_file(move |picked| {
        let _ = tx.send(picked);
    });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|e| ClickSyncError::SourceReadFailed(e.to_string()))?;
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(Some(PickedFile {
        name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        path: path.to_string_lossy().to_string(),
        size,
    }))
}

/// IPC: テキストファイルを（Shift_JIS も含めて）読み込む。台本の取り込み用。
#[tauri::command]
pub async fn cmd_read_text_file(path: String) -> Result<String, ClickSyncError> {
    source::read_text_file(&path)
}

/// IPC: 素材（音声・字幕・台本）を読み込む。音声はここで文字起こしまで行う。
#[tauri::command]
pub async fn cmd_load_source(
    app: AppHandle,
    source: SourceInput,
    topic: String,
) -> Result<VideoData, ClickSyncError> {
    let settings = load_settings(&app)?;
    if settings.gemini_api_key.is_empty() {
        return Err(ClickSyncError::ApiKeyMissing);
    }
    source::load_source(&settings.gemini_api_key, &source, &topic).await
}

/// IPC: トーン判定・サムネイル用プロンプト3パターン・SEOメタデータを生成
#[tauri::command]
pub async fn cmd_generate(app: AppHandle, video_data: VideoData) -> Result<GeneratedAssets, ClickSyncError> {
    let settings = load_settings(&app)?;
    if settings.gemini_api_key.is_empty() {
        return Err(ClickSyncError::ApiKeyMissing);
    }
    generate::generate_assets(&video_data, &settings).await
}

/// IPC: Save user settings
#[tauri::command]
pub async fn cmd_save_settings(
    app: AppHandle,
    settings: AppSettings,
) -> Result<(), ClickSyncError> {
    save_settings(&app, &settings)
}

/// IPC: Load user settings (returns API key masked)
#[tauri::command]
pub async fn cmd_load_settings(app: AppHandle) -> Result<AppSettings, ClickSyncError> {
    let mut s = load_settings(&app)?;
    if !s.gemini_api_key.is_empty() {
        let masked = format!(
            "{}{}",
            "*".repeat(s.gemini_api_key.len().saturating_sub(4)),
            &s.gemini_api_key[s.gemini_api_key.len().saturating_sub(4)..]
        );
        s.gemini_api_key = masked;
    }
    Ok(s)
}

/// IPC: Check if API key is configured
#[tauri::command]
pub async fn cmd_has_api_key(app: AppHandle) -> bool {
    has_api_key(&app)
}

#[tauri::command]
pub async fn cmd_list_gemini_models(
    app: AppHandle,
    api_key: Option<String>,
) -> Result<Vec<GeminiModelInfo>, ClickSyncError> {
    Ok(cmd_refresh_gemini_catalog(app, api_key, Some(true)).await?.models)
}

#[tauri::command]
pub async fn cmd_refresh_gemini_catalog(
    app: AppHandle,
    api_key: Option<String>,
    force: Option<bool>,
) -> Result<GeminiModelCatalog, ClickSyncError> {
    let stored = load_settings(&app)?;
    let key = api_key
        .filter(|k| !k.is_empty() && !k.contains('*'))
        .unwrap_or(stored.gemini_api_key);
    gemini::get_model_catalog(&key, force.unwrap_or(false)).await
}

/// IPC: 直近のリクエストが実際に成功したモデルID（「実行モデル」表示用）
#[tauri::command]
pub async fn cmd_active_model() -> Option<String> {
    gemini::last_successful_model().await
}

#[derive(serde::Serialize)]
pub struct AboutInfo {
    version: String,
    third_party_licenses: &'static str,
}

/// IPC: バージョンと、同梱しているオープンソースのライセンス表記
#[tauri::command]
pub fn cmd_about(app: AppHandle) -> AboutInfo {
    AboutInfo {
        version: app.package_info().version.to_string(),
        third_party_licenses: include_str!("../../resources/THIRD_PARTY_LICENSES.txt"),
    }
}

/// IPC: 見本画像をアプリのデータ領域へ永続保存し、そのパスを返す。
/// webview の File には絶対パスが無いため、中身(base64)を受け取ってコピーする。
#[tauri::command]
pub async fn cmd_save_reference_image(
    app: AppHandle,
    filename: String,
    data_base64: String,
) -> Result<String, ClickSyncError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|e| ClickSyncError::CacheError(format!("base64: {e}")))?;
    // 壊れた画像を保存しないよう、デコードできるか確認する
    image::load_from_memory(&bytes)
        .map_err(|e| ClickSyncError::ReferenceImageFailed(e.to_string()))?;

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ClickSyncError::CacheError(e.to_string()))?
        .join("reference");
    std::fs::create_dir_all(&dir).map_err(|e| ClickSyncError::CacheError(e.to_string()))?;

    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp"))
        .unwrap_or_else(|| "png".to_string());
    let path = dir.join(format!("reference_{}.{ext}", chrono::Utc::now().timestamp_millis()));
    std::fs::write(&path, bytes).map_err(|e| ClickSyncError::CacheError(e.to_string()))?;
    Ok(path.to_string_lossy().to_string())
}

/// IPC: 画像ファイルを `data:` URI として返す（プレビュー用。asset プロトコル設定に依存しない）。
/// ファイルが無い／読めない場合は None。
#[tauri::command]
pub async fn cmd_read_image_data_url(path: String) -> Option<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = std::fs::read(&path).ok()?;
    if bytes.is_empty() || bytes.len() > 25 * 1024 * 1024 {
        return None;
    }
    let mime = match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/jpeg",
    };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

/// store/settings.rs
///
/// Handles persistent storage of user settings using tauri-plugin-store.
/// The Gemini API key is stored in the OS-managed app data directory
/// (never in plaintext in the project folder).
///
/// Store file location:
///   - Windows: %APPDATA%\clicksync\settings.json
///   - macOS:   ~/Library/Application Support/clicksync/settings.json

use crate::types::{AppSettings, ClickSyncError};
use serde_json::json;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_FILE: &str = "settings.json";
const KEY_GEMINI_API_KEY: &str = "gemini_api_key";
const KEY_FIXED_BOILERPLATE: &str = "fixed_boilerplate";
const KEY_MODEL_IMAGE_PATH: &str = "model_image_path";
const KEY_GEMINI_MODEL: &str = "gemini_model";

pub const DEFAULT_GEMINI_MODEL: &str = "auto";

// ─── Public API ───────────────────────────────────────────────────────────────

pub fn load_settings(app: &AppHandle) -> Result<AppSettings, ClickSyncError> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| ClickSyncError::CacheError(format!("store open: {e}")))?;

    let gemini_api_key = store
        .get(KEY_GEMINI_API_KEY)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    let fixed_boilerplate = store
        .get(KEY_FIXED_BOILERPLATE)
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    let model_image_path = store
        .get(KEY_MODEL_IMAGE_PATH)
        .and_then(|v| v.as_str().map(String::from))
        .filter(|s| !s.is_empty());

    let gemini_model = DEFAULT_GEMINI_MODEL.to_string();

    Ok(AppSettings {
        gemini_api_key,
        gemini_model,
        fixed_boilerplate,
        model_image_path,
    })
}

pub fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), ClickSyncError> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| ClickSyncError::CacheError(format!("store open: {e}")))?;

    let mut key = settings.gemini_api_key.clone();
    if key.contains('*') || key.is_empty() {
        if let Some(existing) = store
            .get(KEY_GEMINI_API_KEY)
            .and_then(|v| v.as_str().map(String::from))
        {
            if !existing.is_empty() && (key.contains('*') || key.is_empty()) {
                key = existing;
            }
        }
    }
    store.set(KEY_GEMINI_API_KEY, json!(key));
    store.set(KEY_GEMINI_MODEL, json!(DEFAULT_GEMINI_MODEL));
    store.set(KEY_FIXED_BOILERPLATE, json!(settings.fixed_boilerplate));
    store.set(
        KEY_MODEL_IMAGE_PATH,
        json!(settings.model_image_path.clone().unwrap_or_default()),
    );

    store
        .save()
        .map_err(|e| ClickSyncError::CacheError(format!("store save: {e}")))?;

    Ok(())
}

pub fn has_api_key(app: &AppHandle) -> bool {
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(KEY_GEMINI_API_KEY))
        .and_then(|v| v.as_str().map(|s| !s.is_empty()))
        .unwrap_or(false)
}

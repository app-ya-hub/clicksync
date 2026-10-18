#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod store;
mod types;

use commands::{
    cmd_about, cmd_active_model, cmd_generate, cmd_has_api_key, cmd_list_gemini_models, cmd_load_settings,
    cmd_load_source, cmd_pick_source_file, cmd_read_image_data_url, cmd_read_text_file,
    cmd_refresh_gemini_catalog, cmd_save_reference_image, cmd_save_settings,
};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            cmd_pick_source_file,
            cmd_read_text_file,
            cmd_load_source,
            cmd_generate,
            cmd_save_settings,
            cmd_load_settings,
            cmd_has_api_key,
            cmd_list_gemini_models,
            cmd_refresh_gemini_catalog,
            cmd_active_model,
            cmd_save_reference_image,
            cmd_read_image_data_url,
            cmd_about,
        ])
        .run(tauri::generate_context!())
        .expect("ClickSync起動エラー");
}

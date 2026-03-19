mod api;
mod commands;
mod error;
mod machine;
mod storage;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            activate_license,
            deactivate_license,
            get_license_status,
            get_plugin_catalog,
            install_plugin,
            uninstall_plugin,
            get_installed_plugins,
            check_plugin_updates,
            send_feedback,
            get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

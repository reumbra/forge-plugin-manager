mod api;
mod cowork;
mod commands;
mod error;
mod machine;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|_app| {
            #[cfg(desktop)]
            {
                _app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            activate_license,
            deactivate_license,
            get_license_status,
            get_plugin_catalog,
            install_plugin,
            uninstall_plugin,
            get_installed_plugins,
            check_plugin_updates,
            get_cowork_path,
            set_cowork_path,
            send_feedback,
            get_app_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

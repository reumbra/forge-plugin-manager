use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::api::{ApiClient, FeedbackRequest, LicenseInfo, PluginInfo};
use crate::error::AppError;
use crate::machine;
use crate::storage::{self, InstalledPlugin, TargetInfo};

/// App state managed by Tauri
pub struct AppState {
    pub api: ApiClient,
    pub license_key: Mutex<Option<String>>,
    pub machine_id: String,
}

impl Default for AppState {
    fn default() -> Self {
        // Load persisted license key from config
        let config = storage::load_config().unwrap_or_default();

        Self {
            api: ApiClient::new(),
            license_key: Mutex::new(config.license_key),
            machine_id: machine::get_machine_id(),
        }
    }
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub machine_id: String,
    pub targets: TargetInfo,
    pub config_dir: Option<String>,
    pub os: String,
}

#[derive(Deserialize)]
pub struct InstallRequest {
    pub plugin_name: String,
    pub version: Option<String>,
}

#[derive(Serialize)]
pub struct PluginUpdateInfo {
    pub name: String,
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
}

// --- Commands ---

#[tauri::command]
pub async fn activate_license(
    license_key: String,
    state: State<'_, AppState>,
) -> Result<LicenseInfo, AppError> {
    let info = state
        .api
        .activate(&license_key, &state.machine_id)
        .await?;

    // Persist to memory
    *state.license_key.lock().unwrap() = Some(license_key.clone());

    // Persist to config.json
    let mut config = storage::load_config().unwrap_or_default();
    config.license_key = Some(license_key);
    config.machine_id = Some(state.machine_id.clone());
    config.plan = Some(info.plan.clone());
    config.expires_at = Some(info.expires_at.clone());
    storage::save_config(&config)?;

    Ok(info)
}

#[tauri::command]
pub async fn deactivate_license(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let key = state
        .license_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::License("No license activated".into()))?;

    let result = state.api.deactivate(&key, &state.machine_id).await?;

    // Clear from memory
    *state.license_key.lock().unwrap() = None;

    // Clear from config.json
    let mut config = storage::load_config().unwrap_or_default();
    config.license_key = None;
    config.plan = None;
    config.expires_at = None;
    storage::save_config(&config)?;

    Ok(result)
}

#[tauri::command]
pub async fn get_license_status(
    state: State<'_, AppState>,
) -> Result<LicenseInfo, AppError> {
    let key = state
        .license_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::License("No license activated".into()))?;

    state.api.status(&key, &state.machine_id).await
}

#[tauri::command]
pub async fn get_plugin_catalog(
    state: State<'_, AppState>,
) -> Result<Vec<PluginInfo>, AppError> {
    let key = state
        .license_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::License("No license activated".into()))?;

    state.api.list_plugins(&key, &state.machine_id).await
}

#[tauri::command]
pub async fn install_plugin(
    request: InstallRequest,
    state: State<'_, AppState>,
) -> Result<InstalledPlugin, AppError> {
    let key = state
        .license_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::License("No license activated".into()))?;

    // Get download URL from API
    let download = state
        .api
        .download_plugin(
            &key,
            &state.machine_id,
            &request.plugin_name,
            request.version.as_deref(),
        )
        .await?;

    // Download the zip file
    let zip_data = reqwest::get(&download.download_url)
        .await
        .map_err(AppError::Network)?
        .bytes()
        .await
        .map_err(AppError::Network)?;

    // Install to marketplace directory
    storage::install_plugin_from_zip(
        &request.plugin_name,
        &download.version,
        &zip_data,
    )
}

#[tauri::command]
pub async fn uninstall_plugin(
    plugin_name: String,
) -> Result<(), AppError> {
    storage::uninstall_plugin(&plugin_name)
}

#[tauri::command]
pub async fn get_installed_plugins() -> Result<Vec<InstalledPlugin>, AppError> {
    storage::list_installed()
}

#[tauri::command]
pub async fn check_plugin_updates(
    state: State<'_, AppState>,
) -> Result<Vec<PluginUpdateInfo>, AppError> {
    let key = state
        .license_key
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::License("No license activated".into()))?;

    let installed = storage::list_installed()?;
    let catalog = state.api.list_plugins(&key, &state.machine_id).await?;

    let updates: Vec<PluginUpdateInfo> = installed
        .iter()
        .map(|inst| {
            let latest = catalog
                .iter()
                .find(|p| p.name == inst.name)
                .map(|p| p.latest_version.clone())
                .unwrap_or_else(|| inst.version.clone());

            PluginUpdateInfo {
                name: inst.name.clone(),
                current_version: inst.version.clone(),
                latest_version: latest.clone(),
                has_update: latest != inst.version,
            }
        })
        .collect();

    Ok(updates)
}

#[tauri::command]
pub fn get_cowork_path() -> Option<String> {
    let targets = storage::detect_targets();
    targets.cowork_path
}

#[tauri::command]
pub fn set_cowork_path(_path: String) -> Result<(), AppError> {
    // Legacy command — kept for frontend compat, no-op now
    Ok(())
}

#[tauri::command]
pub async fn send_feedback(
    feedback_type: String,
    message: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, AppError> {
    let key = state.license_key.lock().unwrap().clone();

    state
        .api
        .send_feedback(FeedbackRequest {
            license_key: key,
            feedback_type,
            message,
            metadata: Some(serde_json::json!({
                "source": "plugin-manager",
                "version": env!("CARGO_PKG_VERSION"),
                "os": std::env::consts::OS,
            })),
        })
        .await
}

#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        machine_id: state.machine_id.clone(),
        targets: storage::detect_targets(),
        config_dir: storage::config_dir().ok().map(|p| p.display().to_string()),
        os: std::env::consts::OS.to_string(),
    }
}

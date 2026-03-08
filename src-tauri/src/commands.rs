use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::api::{ApiClient, FeedbackRequest, LicenseInfo, PluginInfo};
use crate::cowork::{self, InstalledPlugin};
use crate::error::AppError;
use crate::machine;

/// App state managed by Tauri
pub struct AppState {
    pub api: ApiClient,
    pub license_key: Mutex<Option<String>>,
    pub machine_id: String,
    pub cowork_path: Mutex<Option<PathBuf>>,
}

impl Default for AppState {
    fn default() -> Self {
        // Try to auto-detect cowork path
        let cowork_path = cowork::detect_cowork_base().ok();

        Self {
            api: ApiClient::new(),
            license_key: Mutex::new(None),
            machine_id: machine::get_machine_id(),
            cowork_path: Mutex::new(cowork_path),
        }
    }
}

#[derive(Serialize)]
pub struct AppInfo {
    pub version: String,
    pub machine_id: String,
    pub cowork_detected: bool,
    pub cowork_path: Option<String>,
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

    *state.license_key.lock().unwrap() = Some(license_key);
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
    *state.license_key.lock().unwrap() = None;
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

    let cowork_path = state
        .cowork_path
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::CoworkNotFound("Cowork path not configured".into()))?;

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
        .map_err(|e| AppError::Network(e))?
        .bytes()
        .await
        .map_err(|e| AppError::Network(e))?;

    // Install to Cowork directory
    cowork::install_plugin_from_zip(
        &cowork_path,
        &request.plugin_name,
        &download.version,
        &zip_data,
    )
}

#[tauri::command]
pub async fn uninstall_plugin(
    plugin_name: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let cowork_path = state
        .cowork_path
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::CoworkNotFound("Cowork path not configured".into()))?;

    cowork::uninstall_plugin(&cowork_path, &plugin_name)
}

#[tauri::command]
pub async fn get_installed_plugins(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledPlugin>, AppError> {
    let cowork_path = state
        .cowork_path
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::CoworkNotFound("Cowork path not configured".into()))?;

    cowork::list_installed(&cowork_path)
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

    let cowork_path = state
        .cowork_path
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| AppError::CoworkNotFound("Cowork path not configured".into()))?;

    let installed = cowork::list_installed(&cowork_path)?;
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
pub fn get_cowork_path(state: State<'_, AppState>) -> Option<String> {
    state
        .cowork_path
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn set_cowork_path(path: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        return Err(AppError::CoworkNotFound(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }
    *state.cowork_path.lock().unwrap() = Some(path);
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
    let cowork = state.cowork_path.lock().unwrap();
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        machine_id: state.machine_id.clone(),
        cowork_detected: cowork.is_some(),
        cowork_path: cowork.as_ref().map(|p| p.display().to_string()),
        os: std::env::consts::OS.to_string(),
    }
}

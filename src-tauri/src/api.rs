use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const API_BASE: &str = "https://api.reumbra.com/velvet";

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

// --- Request/Response types ---

#[derive(Debug, Serialize)]
pub struct ActivateRequest {
    pub license_key: String,
    pub machine_id: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LicenseInfo {
    pub license_key: String,
    pub plan: String,
    pub is_active: bool,
    pub expires_at: String,
    pub machines: Vec<MachineInfo>,
    pub max_machines: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MachineInfo {
    pub machine_id: String,
    pub activated_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub description: Option<String>,
    pub latest_version: String,
    pub category: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DownloadRequest {
    pub license_key: String,
    pub machine_id: String,
    pub plugin_name: String,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadResponse {
    pub download_url: String,
    pub version: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersionInfo {
    pub version: String,
    pub released_at: String,
    pub changelog: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FeedbackRequest {
    pub license_key: Option<String>,
    pub feedback_type: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: String,
}

// --- Implementation ---

impl ApiClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: API_BASE.to_string(),
        }
    }

    pub async fn activate(
        &self,
        license_key: &str,
        machine_id: &str,
    ) -> Result<LicenseInfo, AppError> {
        let resp = self
            .client
            .post(format!("{}/auth/activate", self.base_url))
            .json(&ActivateRequest {
                license_key: license_key.to_string(),
                machine_id: machine_id.to_string(),
            })
            .send()
            .await?;

        self.parse_response(resp).await
    }

    pub async fn deactivate(
        &self,
        license_key: &str,
        machine_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        let resp = self
            .client
            .post(format!("{}/auth/deactivate", self.base_url))
            .json(&serde_json::json!({
                "license_key": license_key,
                "machine_id": machine_id,
            }))
            .send()
            .await?;

        self.parse_response(resp).await
    }

    pub async fn status(
        &self,
        license_key: &str,
        machine_id: &str,
    ) -> Result<LicenseInfo, AppError> {
        let resp = self
            .client
            .get(format!("{}/auth/status", self.base_url))
            .query(&[("license_key", license_key), ("machine_id", machine_id)])
            .send()
            .await?;

        self.parse_response(resp).await
    }

    pub async fn list_plugins(
        &self,
        license_key: &str,
        machine_id: &str,
    ) -> Result<Vec<PluginInfo>, AppError> {
        let resp = self
            .client
            .get(format!("{}/plugins/list", self.base_url))
            .query(&[("license_key", license_key), ("machine_id", machine_id)])
            .send()
            .await?;

        let body: serde_json::Value = self.parse_response(resp).await?;
        let plugins: Vec<PluginInfo> =
            serde_json::from_value(body["plugins"].clone()).unwrap_or_default();
        Ok(plugins)
    }

    pub async fn download_plugin(
        &self,
        license_key: &str,
        machine_id: &str,
        plugin_name: &str,
        version: Option<&str>,
    ) -> Result<DownloadResponse, AppError> {
        let mut req = serde_json::json!({
            "license_key": license_key,
            "machine_id": machine_id,
            "plugin_name": plugin_name,
        });

        if let Some(v) = version {
            req["version"] = serde_json::Value::String(v.to_string());
        }

        let resp = self
            .client
            .post(format!("{}/plugins/download", self.base_url))
            .json(&req)
            .send()
            .await?;

        self.parse_response(resp).await
    }

    pub async fn get_versions(
        &self,
        plugin_name: &str,
        license_key: &str,
        machine_id: &str,
    ) -> Result<Vec<VersionInfo>, AppError> {
        let resp = self
            .client
            .get(format!("{}/plugins/versions/{}", self.base_url, plugin_name))
            .query(&[("license_key", license_key), ("machine_id", machine_id)])
            .send()
            .await?;

        let body: serde_json::Value = self.parse_response(resp).await?;
        let versions: Vec<VersionInfo> =
            serde_json::from_value(body["versions"].clone()).unwrap_or_default();
        Ok(versions)
    }

    pub async fn send_feedback(&self, feedback: FeedbackRequest) -> Result<serde_json::Value, AppError> {
        let resp = self
            .client
            .post(format!("{}/feedback", self.base_url))
            .json(&feedback)
            .send()
            .await?;

        self.parse_response(resp).await
    }

    async fn parse_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, AppError> {
        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            let err_msg = serde_json::from_str::<ApiErrorResponse>(&text)
                .map(|e| e.error)
                .unwrap_or(text);
            return Err(AppError::Api(err_msg));
        }

        serde_json::from_str(&text).map_err(|e| AppError::Json(e))
    }
}

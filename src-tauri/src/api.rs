use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const API_BASE: &str = "https://api.reumbra.com/velvet";

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

// --- Public types (used by commands.rs) ---

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
    #[serde(alias = "current_version")]
    pub latest_version: String,
    pub category: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DownloadResponse {
    pub url: String,
    pub plugin_name: String,
    pub version: String,
    pub expires_in: u64,
}

#[derive(Debug, Serialize)]
pub struct FeedbackRequest {
    pub license_key: Option<String>,
    pub machine_id: String,
    pub feedback_type: String,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

// --- Internal API response types ---

#[derive(Debug, Serialize)]
struct ActivateRequest {
    license_key: String,
    machine_id: String,
}

#[derive(Debug, Deserialize)]
struct ActivateResponse {
    #[allow(dead_code)]
    success: bool,
    license: ActivateLicense,
}

#[derive(Debug, Deserialize)]
struct ActivateLicense {
    plan: String,
    expires_at: String,
    #[allow(dead_code)]
    machines_used: i32,
    max_machines: i32,
}

#[derive(Debug, Deserialize)]
struct StatusResponse {
    #[allow(dead_code)]
    valid: bool,
    license: StatusLicense,
}

#[derive(Debug, Deserialize)]
struct StatusLicense {
    plan: String,
    expires_at: String,
    is_active: bool,
    machines: Vec<StatusMachine>,
    max_machines: Option<i32>,
    #[allow(dead_code)]
    allowed_plugins: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StatusMachine {
    machine_id: String,
    activated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PluginListResponse {
    plugins: Vec<PluginInfo>,
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

        let data: ActivateResponse = self.parse_response(resp).await?;

        Ok(LicenseInfo {
            license_key: license_key.to_string(),
            plan: data.license.plan,
            is_active: true,
            expires_at: data.license.expires_at,
            machines: vec![MachineInfo {
                machine_id: machine_id.to_string(),
                activated_at: chrono::Utc::now().to_rfc3339(),
            }],
            max_machines: data.license.max_machines,
        })
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

        let data: StatusResponse = self.parse_response(resp).await?;

        Ok(LicenseInfo {
            license_key: license_key.to_string(),
            plan: data.license.plan,
            is_active: data.license.is_active,
            expires_at: data.license.expires_at,
            machines: data
                .license
                .machines
                .into_iter()
                .map(|m| MachineInfo {
                    machine_id: m.machine_id,
                    activated_at: m.activated_at.unwrap_or_default(),
                })
                .collect(),
            max_machines: data.license.max_machines.unwrap_or(3),
        })
    }

    pub async fn list_plugins(
        &self,
        license_key: &str,
        machine_id: &str,
    ) -> Result<Vec<PluginInfo>, AppError> {
        let resp = self
            .client
            .get(format!("{}/plugins/list", self.base_url))
            .header("x-license-key", license_key)
            .header("x-machine-id", machine_id)
            .send()
            .await?;

        let body: PluginListResponse = self.parse_response(resp).await?;
        Ok(body.plugins)
    }

    pub async fn download_plugin(
        &self,
        license_key: &str,
        machine_id: &str,
        plugin_name: &str,
        version: Option<&str>,
    ) -> Result<DownloadResponse, AppError> {
        let mut req = serde_json::json!({
            "plugin_name": plugin_name,
        });

        if let Some(v) = version {
            req["version"] = serde_json::Value::String(v.to_string());
        }

        let resp = self
            .client
            .post(format!("{}/plugins/download", self.base_url))
            .header("x-license-key", license_key)
            .header("x-machine-id", machine_id)
            .json(&req)
            .send()
            .await?;

        self.parse_response(resp).await
    }

    pub async fn send_feedback(
        &self,
        feedback: FeedbackRequest,
    ) -> Result<serde_json::Value, AppError> {
        let mut builder = self
            .client
            .post(format!("{}/feedback", self.base_url));

        if let Some(ref key) = feedback.license_key {
            builder = builder
                .header("x-license-key", key)
                .header("x-machine-id", &feedback.machine_id);
        }

        let body = serde_json::json!({
            "feedback_type": "direct_feedback",
            "user_comments": feedback.message,
            "plugin_name": "plugin-manager",
            "plugin_version": env!("CARGO_PKG_VERSION"),
        });

        let resp = builder.json(&body).send().await?;
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

        serde_json::from_str(&text).map_err(AppError::Json)
    }
}

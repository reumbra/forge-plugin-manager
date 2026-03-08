use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const MARKETPLACE_NAME: &str = "reumbra-plugins";

/// Represents installed_plugins.json
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledPluginsRegistry {
    pub version: u32,
    pub plugins: std::collections::HashMap<String, Vec<PluginInstallation>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginInstallation {
    pub scope: String,
    #[serde(rename = "installPath")]
    pub install_path: String,
    pub version: String,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
    #[serde(rename = "lastUpdated")]
    pub last_updated: String,
    #[serde(rename = "gitCommitSha", skip_serializing_if = "Option::is_none")]
    pub git_commit_sha: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<PluginAuthor>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginAuthor {
    pub name: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub marketplace: String,
    pub installed_at: String,
    pub install_path: String,
}

/// Detect Cowork plugins directory.
/// Windows: %APPDATA%/Claude/local-agent-mode-sessions/
/// macOS: ~/Library/Application Support/Claude/local-agent-mode-sessions/
/// Linux: ~/.config/Claude/local-agent-mode-sessions/
pub fn detect_cowork_base() -> Result<PathBuf, AppError> {
    // dirs::config_dir() returns the correct path per OS:
    // Windows: %APPDATA%, macOS: ~/Library/Application Support, Linux: ~/.config
    let base = dirs::config_dir();

    let base = base.ok_or_else(|| AppError::CoworkNotFound("Cannot determine config directory".into()))?;
    let claude_dir = base.join("Claude").join("local-agent-mode-sessions");

    if !claude_dir.exists() {
        return Err(AppError::CoworkNotFound(format!(
            "Claude Cowork directory not found at {}",
            claude_dir.display()
        )));
    }

    // Find the session directory containing cowork_plugins
    find_cowork_plugins_dir(&claude_dir)
}

/// Recursively search for cowork_plugins directory within session dirs
fn find_cowork_plugins_dir(sessions_dir: &Path) -> Result<PathBuf, AppError> {
    for session_entry in fs::read_dir(sessions_dir).map_err(|e| {
        AppError::CoworkNotFound(format!("Cannot read sessions dir: {}", e))
    })? {
        let session_entry = session_entry.map_err(|e| {
            AppError::CoworkNotFound(format!("Cannot read entry: {}", e))
        })?;

        if !session_entry.path().is_dir() {
            continue;
        }

        // Look inside org subdirectories
        if let Ok(org_entries) = fs::read_dir(session_entry.path()) {
            for org_entry in org_entries.flatten() {
                let cowork_path = org_entry.path().join("cowork_plugins");
                if cowork_path.exists() && cowork_path.is_dir() {
                    return Ok(cowork_path);
                }
            }
        }
    }

    Err(AppError::CoworkNotFound(
        "No cowork_plugins directory found in any session".into(),
    ))
}

/// Read the installed plugins registry
pub fn read_registry(cowork_path: &Path) -> Result<InstalledPluginsRegistry, AppError> {
    let registry_path = cowork_path.join("installed_plugins.json");

    if !registry_path.exists() {
        return Ok(InstalledPluginsRegistry {
            version: 2,
            plugins: std::collections::HashMap::new(),
        });
    }

    let content = fs::read_to_string(&registry_path)?;
    let registry: InstalledPluginsRegistry = serde_json::from_str(&content)?;
    Ok(registry)
}

/// Write the installed plugins registry
pub fn write_registry(
    cowork_path: &Path,
    registry: &InstalledPluginsRegistry,
) -> Result<(), AppError> {
    let registry_path = cowork_path.join("installed_plugins.json");
    let content = serde_json::to_string_pretty(registry)?;
    fs::write(&registry_path, content)?;
    Ok(())
}

/// Install a plugin from a zip file into the Cowork directory
pub fn install_plugin_from_zip(
    cowork_path: &Path,
    plugin_name: &str,
    version: &str,
    zip_data: &[u8],
) -> Result<InstalledPlugin, AppError> {
    let cache_dir = cowork_path
        .join("cache")
        .join(MARKETPLACE_NAME)
        .join(plugin_name)
        .join(version);

    // Remove existing version if present
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }

    // Create directory and extract zip
    fs::create_dir_all(&cache_dir)?;
    extract_zip(zip_data, &cache_dir)?;

    // Read plugin manifest to get description
    let manifest_path = cache_dir.join(".claude-plugin").join("plugin.json");
    let description = if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path)?;
        let manifest: PluginManifest = serde_json::from_str(&content)?;
        manifest.description
    } else {
        String::new()
    };

    // Update registry
    let mut registry = read_registry(cowork_path)?;
    let key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    let now = Utc::now().to_rfc3339();

    // Build a relative installPath matching Cowork's format
    let install_path = format!(
        "mnt/.claude/cowork_plugins/cache/{}/{}/{}",
        MARKETPLACE_NAME, plugin_name, version
    );

    let installation = PluginInstallation {
        scope: "user".to_string(),
        install_path: install_path.clone(),
        version: version.to_string(),
        installed_at: now.clone(),
        last_updated: now.clone(),
        git_commit_sha: None,
    };

    registry.plugins.insert(key, vec![installation]);
    write_registry(cowork_path, &registry)?;

    Ok(InstalledPlugin {
        name: plugin_name.to_string(),
        version: version.to_string(),
        description,
        marketplace: MARKETPLACE_NAME.to_string(),
        installed_at: now,
        install_path,
    })
}

/// Uninstall a plugin
pub fn uninstall_plugin(cowork_path: &Path, plugin_name: &str) -> Result<(), AppError> {
    // Remove from cache
    let cache_dir = cowork_path.join("cache").join(MARKETPLACE_NAME).join(plugin_name);
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }

    // Remove from registry
    let mut registry = read_registry(cowork_path)?;
    let key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    registry.plugins.remove(&key);
    write_registry(cowork_path, &registry)?;

    Ok(())
}

/// List all installed Reumbra plugins
pub fn list_installed(cowork_path: &Path) -> Result<Vec<InstalledPlugin>, AppError> {
    let registry = read_registry(cowork_path)?;
    let mut plugins = Vec::new();

    for (key, installations) in &registry.plugins {
        // Only show our plugins
        if !key.ends_with(&format!("@{}", MARKETPLACE_NAME)) {
            continue;
        }

        let plugin_name = key.split('@').next().unwrap_or(key);

        for inst in installations {
            // Try to read manifest for description
            let cache_dir = cowork_path
                .join("cache")
                .join(MARKETPLACE_NAME)
                .join(plugin_name)
                .join(&inst.version);

            let description = cache_dir
                .join(".claude-plugin")
                .join("plugin.json")
                .pipe_read_manifest();

            plugins.push(InstalledPlugin {
                name: plugin_name.to_string(),
                version: inst.version.clone(),
                description,
                marketplace: MARKETPLACE_NAME.to_string(),
                installed_at: inst.installed_at.clone(),
                install_path: inst.install_path.clone(),
            });
        }
    }

    Ok(plugins)
}

/// Extract zip data to a directory
fn extract_zip(data: &[u8], dest: &Path) -> Result<(), AppError> {
    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Skip macOS resource fork files
        if name.starts_with("__MACOSX") || name.contains(".DS_Store") {
            continue;
        }

        let out_path = dest.join(&name);

        if file.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

/// Helper trait for reading plugin manifest
trait ReadManifest {
    fn pipe_read_manifest(&self) -> String;
}

impl ReadManifest for PathBuf {
    fn pipe_read_manifest(&self) -> String {
        fs::read_to_string(self)
            .ok()
            .and_then(|c| serde_json::from_str::<PluginManifest>(&c).ok())
            .map(|m| m.description)
            .unwrap_or_default()
    }
}

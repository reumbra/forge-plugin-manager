use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

const MARKETPLACE_NAME: &str = "reumbra";
const APP_DIR_NAME: &str = "forge-devkit";

// --- Config ---

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ForgeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,
    #[serde(default)]
    pub installed_plugins: HashMap<String, InstalledPluginEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledPluginEntry {
    pub version: String,
    pub installed_at: String,
}

// --- Paths ---

/// Get the forge-devkit config directory using OS-standard paths.
/// Windows: %APPDATA%/forge-devkit
/// macOS:   ~/Library/Application Support/forge-devkit
/// Linux:   ~/.config/forge-devkit
pub fn config_dir() -> Result<PathBuf, AppError> {
    dirs::config_dir()
        .map(|d| d.join(APP_DIR_NAME))
        .ok_or_else(|| AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot determine config directory",
        )))
}

/// Get the forge-devkit cache directory.
/// Windows: %LOCALAPPDATA%/forge-devkit/cache
/// macOS:   ~/Library/Caches/forge-devkit
/// Linux:   ~/.cache/forge-devkit
#[allow(dead_code)]
pub fn cache_dir() -> Result<PathBuf, AppError> {
    dirs::cache_dir()
        .map(|d| d.join(APP_DIR_NAME))
        .ok_or_else(|| AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot determine cache directory",
        )))
}

/// Path to the marketplace directory
pub fn marketplace_dir() -> Result<PathBuf, AppError> {
    Ok(config_dir()?.join("marketplace"))
}

/// Path to config.json
pub fn config_path() -> Result<PathBuf, AppError> {
    Ok(config_dir()?.join("config.json"))
}

// --- Config read/write ---

pub fn load_config() -> Result<ForgeConfig, AppError> {
    let path = config_path()?;

    // Try legacy path migration first
    if !path.exists() {
        migrate_legacy_config(&path)?;
    }

    if !path.exists() {
        return Ok(ForgeConfig::default());
    }

    let content = fs::read_to_string(&path)?;
    let config: ForgeConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_config(config: &ForgeConfig) -> Result<(), AppError> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(config)?;
    fs::write(&path, content)?;
    Ok(())
}

/// One-time migration from ~/.forge/config.json to OS-standard path
fn migrate_legacy_config(new_path: &Path) -> Result<(), AppError> {
    let home = dirs::home_dir().ok_or_else(|| AppError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "Cannot determine home directory",
    )))?;

    let legacy_config = home.join(".forge").join("config.json");
    if !legacy_config.exists() {
        return Ok(());
    }

    log::info!("Migrating legacy config from {}", legacy_config.display());

    // Read legacy config
    let content = fs::read_to_string(&legacy_config)?;

    // Write to new location
    if let Some(parent) = new_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(new_path, &content)?;

    // Migrate marketplace if it exists
    let legacy_marketplace = home.join(".forge").join("marketplace");
    if legacy_marketplace.exists() {
        let new_marketplace = marketplace_dir()?;
        if !new_marketplace.exists() {
            copy_dir_recursive(&legacy_marketplace, &new_marketplace)?;
            log::info!("Migrated marketplace to {}", new_marketplace.display());
        }
    }

    log::info!("Legacy migration complete");
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), AppError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// --- Target detection ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallTarget {
    ClaudeCode,
    ClaudeCowork,
}

#[derive(Debug, Serialize, Clone)]
pub struct TargetInfo {
    pub claude_code: bool,
    pub claude_cowork: bool,
    pub claude_code_path: Option<String>,
    pub cowork_path: Option<String>,
}

pub fn detect_targets() -> TargetInfo {
    let home = dirs::home_dir();

    // Claude Code: ~/.claude/
    let claude_code_path = home.as_ref().map(|h| h.join(".claude"));
    let claude_code = claude_code_path.as_ref().is_some_and(|p| p.exists());

    // Claude Cowork: {config_dir}/Claude/local-agent-mode-sessions/{session}/{user}/cowork_plugins/
    let cowork_plugins = find_cowork_plugins_dir();
    let claude_cowork = cowork_plugins.is_some();

    TargetInfo {
        claude_code,
        claude_cowork,
        claude_code_path: if claude_code { claude_code_path.map(|p| p.display().to_string()) } else { None },
        cowork_path: cowork_plugins.map(|p| p.display().to_string()),
    }
}

/// Find the cowork_plugins directory by scanning local-agent-mode-sessions/{session}/{user}/
/// Returns the first cowork_plugins dir found (typically there's one session with one user).
pub fn find_cowork_plugins_dir() -> Option<PathBuf> {
    let config = dirs::config_dir()?;
    let sessions_dir = config.join("Claude").join("local-agent-mode-sessions");
    if !sessions_dir.exists() {
        return None;
    }

    // Scan: sessions_dir/{sessionId}/{userId}/cowork_plugins/
    for session_entry in fs::read_dir(&sessions_dir).ok()? {
        let session_entry = session_entry.ok()?;
        let session_path = session_entry.path();
        if !session_path.is_dir() || session_entry.file_name() == "skills-plugin" {
            continue;
        }

        for user_entry in fs::read_dir(&session_path).ok()? {
            let user_entry = user_entry.ok()?;
            let user_path = user_entry.path();
            if !user_path.is_dir() {
                continue;
            }

            let cowork_plugins = user_path.join("cowork_plugins");
            if cowork_plugins.exists() {
                return Some(cowork_plugins);
            }
        }
    }

    None
}

/// Claude Code plugins directory: ~/.claude/plugins/
#[allow(dead_code)]
pub fn claude_code_plugins_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("plugins"))
}

// --- Plugin installation ---

/// Marketplace manifest for Claude Code discovery
#[derive(Debug, Serialize, Deserialize)]
struct MarketplaceManifest {
    name: String,
    owner: String,
    #[serde(default)]
    plugins: Vec<MarketplacePlugin>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MarketplacePlugin {
    name: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub marketplace: String,
    pub installed_at: String,
    pub install_path: String,
    pub targets: Vec<String>,
}

pub fn install_plugin_from_zip(
    plugin_name: &str,
    version: &str,
    zip_data: &[u8],
    target: InstallTarget,
) -> Result<InstalledPlugin, AppError> {
    // Always extract to our own marketplace dir first (source of truth)
    let mkt_dir = marketplace_dir()?;
    let plugin_dir = mkt_dir.join("plugins").join(plugin_name);

    if plugin_dir.exists() {
        fs::remove_dir_all(&plugin_dir)?;
    }
    fs::create_dir_all(&plugin_dir)?;
    extract_zip(zip_data, &plugin_dir)?;

    // Read plugin manifest for description
    let manifest_path = plugin_dir.join(".claude-plugin").join("plugin.json");
    let description = if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path)?;
        serde_json::from_str::<PluginManifest>(&content)
            .map(|m| m.description)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Update our marketplace.json
    update_marketplace_manifest(plugin_name, version, &description)?;

    // Integrate with selected target
    let mut installed_targets = Vec::new();
    match target {
        InstallTarget::ClaudeCode => {
            integrate_claude_code(plugin_name)?;
            installed_targets.push("claude-code".to_string());
        }
        InstallTarget::ClaudeCowork => {
            integrate_cowork(plugin_name, version, &description, &plugin_dir)?;
            installed_targets.push("claude-cowork".to_string());
        }
    }

    // Update our config (tracks what we installed)
    let mut config = load_config()?;
    let now = Utc::now().to_rfc3339();
    config.installed_plugins.insert(
        plugin_name.to_string(),
        InstalledPluginEntry {
            version: version.to_string(),
            installed_at: now.clone(),
        },
    );
    save_config(&config)?;

    Ok(InstalledPlugin {
        name: plugin_name.to_string(),
        version: version.to_string(),
        description,
        marketplace: MARKETPLACE_NAME.to_string(),
        installed_at: now,
        install_path: plugin_dir.display().to_string(),
        targets: installed_targets,
    })
}

fn update_marketplace_manifest(
    plugin_name: &str,
    version: &str,
    description: &str,
) -> Result<(), AppError> {
    let mkt_dir = marketplace_dir()?;
    let manifest_path = mkt_dir.join(".claude-plugin").join("marketplace.json");

    fs::create_dir_all(manifest_path.parent().unwrap())?;

    let mut manifest = if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path)?;
        serde_json::from_str(&content)?
    } else {
        MarketplaceManifest {
            name: MARKETPLACE_NAME.to_string(),
            owner: "Reumbra".to_string(),
            plugins: Vec::new(),
        }
    };

    // Update or add plugin entry
    let source = format!("./plugins/{}", plugin_name);
    if let Some(existing) = manifest.plugins.iter_mut().find(|p| p.name == plugin_name) {
        existing.source = source;
        existing.version = Some(version.to_string());
        existing.description = Some(description.to_string());
    } else {
        manifest.plugins.push(MarketplacePlugin {
            name: plugin_name.to_string(),
            source,
            description: Some(description.to_string()),
            version: Some(version.to_string()),
        });
    }

    let content = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, content)?;
    Ok(())
}

/// Register marketplace and enable plugin in Claude Code
fn integrate_claude_code(plugin_name: &str) -> Result<(), AppError> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(()), // Can't integrate without home dir
    };

    let claude_dir = home.join(".claude");
    if !claude_dir.exists() {
        log::warn!("Claude Code not detected, skipping integration");
        return Ok(());
    }

    let plugins_dir = claude_dir.join("plugins");
    fs::create_dir_all(&plugins_dir)?;

    // 1. Register marketplace in known_marketplaces.json
    let km_path = plugins_dir.join("known_marketplaces.json");
    let mut km: serde_json::Value = if km_path.exists() {
        let content = fs::read_to_string(&km_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let mkt_path = marketplace_dir()?.display().to_string();
    // Always update marketplace path — ensures Claude Code points to the correct
    // directory even if a previous CLI version registered a legacy path
    km[MARKETPLACE_NAME] = serde_json::json!({
        "source": { "source": "directory", "path": mkt_path },
        "installLocation": mkt_path,
        "lastUpdated": Utc::now().to_rfc3339()
    });
    fs::write(&km_path, serde_json::to_string_pretty(&km)?)?;
    log::info!("Registered marketplace in Claude Code at {}", mkt_path);

    // 2. Enable plugin in settings.json
    let settings_path = claude_dir.join("settings.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    if settings.get("enabledPlugins").is_none() {
        settings["enabledPlugins"] = serde_json::json!({});
    }
    settings["enabledPlugins"][&plugin_key] = serde_json::Value::Bool(true);
    fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    // 3. Invalidate stale cache
    let cache_plugin = plugins_dir.join("cache").join(MARKETPLACE_NAME).join(plugin_name);
    if cache_plugin.exists() {
        let _ = fs::remove_dir_all(&cache_plugin);
        log::info!("Cleared stale cache for {}", plugin_name);
    }

    let active_copy = plugins_dir.join(plugin_name);
    if active_copy.exists() {
        let _ = fs::remove_dir_all(&active_copy);
        log::info!("Cleared stale active copy for {}", plugin_name);
    }

    // Remove from installed_plugins.json
    let ip_path = plugins_dir.join("installed_plugins.json");
    if ip_path.exists() {
        let content = fs::read_to_string(&ip_path)?;
        let mut ip: serde_json::Value = serde_json::from_str(&content)?;
        if let Some(plugins) = ip.get_mut("plugins") {
            if plugins.is_object() {
                plugins.as_object_mut().unwrap().remove(&plugin_key);
                fs::write(&ip_path, serde_json::to_string_pretty(&ip)?)?;
            }
        }
    }

    Ok(())
}

/// Register marketplace and install plugin in Claude Cowork
fn integrate_cowork(
    plugin_name: &str,
    version: &str,
    description: &str,
    source_dir: &Path,
) -> Result<(), AppError> {
    let cowork_dir = match find_cowork_plugins_dir() {
        Some(d) => d,
        None => {
            log::warn!("Cowork not detected, skipping integration");
            return Ok(());
        }
    };

    // 1. Copy plugin to marketplaces/reumbra/{plugin_name}/
    let mkt_plugin_dir = cowork_dir.join("marketplaces").join(MARKETPLACE_NAME).join(plugin_name);
    if mkt_plugin_dir.exists() {
        fs::remove_dir_all(&mkt_plugin_dir)?;
    }
    copy_dir_recursive(source_dir, &mkt_plugin_dir)?;

    // 2. Update marketplace.json inside cowork marketplaces
    let mkt_manifest_path = cowork_dir
        .join("marketplaces")
        .join(MARKETPLACE_NAME)
        .join(".claude-plugin")
        .join("marketplace.json");
    fs::create_dir_all(mkt_manifest_path.parent().unwrap())?;

    let mut manifest = if mkt_manifest_path.exists() {
        let content = fs::read_to_string(&mkt_manifest_path)?;
        serde_json::from_str(&content)?
    } else {
        MarketplaceManifest {
            name: MARKETPLACE_NAME.to_string(),
            owner: "Reumbra".to_string(),
            plugins: Vec::new(),
        }
    };

    let source = format!("./{}", plugin_name);
    if let Some(existing) = manifest.plugins.iter_mut().find(|p| p.name == plugin_name) {
        existing.source = source;
        existing.version = Some(version.to_string());
        existing.description = Some(description.to_string());
    } else {
        manifest.plugins.push(MarketplacePlugin {
            name: plugin_name.to_string(),
            source,
            description: Some(description.to_string()),
            version: Some(version.to_string()),
        });
    }
    fs::write(&mkt_manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    // 3. Copy to cache/reumbra/{plugin_name}/{version}/
    let cache_dir = cowork_dir
        .join("cache")
        .join(MARKETPLACE_NAME)
        .join(plugin_name)
        .join(version);
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }
    copy_dir_recursive(source_dir, &cache_dir)?;

    // 4. Register in known_marketplaces.json (relative paths with mnt/ prefix)
    let km_path = cowork_dir.join("known_marketplaces.json");
    let mut km: serde_json::Value = if km_path.exists() {
        let content = fs::read_to_string(&km_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let mkt_rel_path = format!("mnt/.claude/cowork_plugins/marketplaces/{}", MARKETPLACE_NAME);
    km[MARKETPLACE_NAME] = serde_json::json!({
        "source": { "source": "directory", "path": mkt_rel_path },
        "installLocation": mkt_rel_path,
        "lastUpdated": Utc::now().to_rfc3339()
    });
    fs::write(&km_path, serde_json::to_string_pretty(&km)?)?;

    // 5. Add to installed_plugins.json
    let ip_path = cowork_dir.join("installed_plugins.json");
    let mut ip: serde_json::Value = if ip_path.exists() {
        let content = fs::read_to_string(&ip_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({ "version": 2, "plugins": {} })
    };

    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    let cache_rel_path = format!(
        "mnt/.claude/cowork_plugins/cache/{}/{}/{}",
        MARKETPLACE_NAME, plugin_name, version
    );
    ip["plugins"][&plugin_key] = serde_json::json!([{
        "scope": "user",
        "installPath": cache_rel_path,
        "version": version,
        "installedAt": Utc::now().to_rfc3339(),
        "lastUpdated": Utc::now().to_rfc3339()
    }]);
    fs::write(&ip_path, serde_json::to_string_pretty(&ip)?)?;

    log::info!("Integrated {} into Cowork", plugin_name);
    Ok(())
}

pub fn uninstall_plugin(plugin_name: &str, target: InstallTarget) -> Result<(), AppError> {
    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);

    match target {
        InstallTarget::ClaudeCode => {
            // Remove from our marketplace
            let mkt_dir = marketplace_dir()?;
            let plugin_dir = mkt_dir.join("plugins").join(plugin_name);
            if plugin_dir.exists() {
                fs::remove_dir_all(&plugin_dir)?;
            }

            // Remove from marketplace.json
            let manifest_path = mkt_dir.join(".claude-plugin").join("marketplace.json");
            if manifest_path.exists() {
                let content = fs::read_to_string(&manifest_path)?;
                let mut manifest: MarketplaceManifest = serde_json::from_str(&content)?;
                manifest.plugins.retain(|p| p.name != plugin_name);
                fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
            }

            // Disable in Claude Code settings.json
            if let Some(home) = dirs::home_dir() {
                let settings_path = home.join(".claude").join("settings.json");
                if settings_path.exists() {
                    let content = fs::read_to_string(&settings_path)?;
                    let mut settings: serde_json::Value = serde_json::from_str(&content)?;
                    if let Some(ep) = settings.get_mut("enabledPlugins") {
                        if let Some(obj) = ep.as_object_mut() {
                            obj.remove(&plugin_key);
                        }
                    }
                    fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
                }
            }
        }
        InstallTarget::ClaudeCowork => {
            if let Some(cowork_dir) = find_cowork_plugins_dir() {
                // Remove from marketplaces/reumbra/{plugin}
                let mkt_plugin_dir = cowork_dir.join("marketplaces").join(MARKETPLACE_NAME).join(plugin_name);
                if mkt_plugin_dir.exists() {
                    fs::remove_dir_all(&mkt_plugin_dir)?;
                }

                // Remove from cache/reumbra/{plugin}
                let cache_plugin_dir = cowork_dir.join("cache").join(MARKETPLACE_NAME).join(plugin_name);
                if cache_plugin_dir.exists() {
                    fs::remove_dir_all(&cache_plugin_dir)?;
                }

                // Remove from installed_plugins.json
                let ip_path = cowork_dir.join("installed_plugins.json");
                if ip_path.exists() {
                    let content = fs::read_to_string(&ip_path)?;
                    let mut ip: serde_json::Value = serde_json::from_str(&content)?;
                    if let Some(plugins) = ip.get_mut("plugins") {
                        if let Some(obj) = plugins.as_object_mut() {
                            obj.remove(&plugin_key);
                        }
                    }
                    fs::write(&ip_path, serde_json::to_string_pretty(&ip)?)?;
                }

                // Update marketplace.json
                let mkt_manifest = cowork_dir
                    .join("marketplaces")
                    .join(MARKETPLACE_NAME)
                    .join(".claude-plugin")
                    .join("marketplace.json");
                if mkt_manifest.exists() {
                    let content = fs::read_to_string(&mkt_manifest)?;
                    let mut manifest: MarketplaceManifest = serde_json::from_str(&content)?;
                    manifest.plugins.retain(|p| p.name != plugin_name);
                    fs::write(&mkt_manifest, serde_json::to_string_pretty(&manifest)?)?;
                }
            }
        }
    }

    // Always remove from our own config
    let mut config = load_config()?;
    config.installed_plugins.remove(plugin_name);
    save_config(&config)?;

    Ok(())
}

/// List installed plugins from marketplace directory, with per-target status
pub fn list_installed() -> Result<Vec<InstalledPlugin>, AppError> {
    let config = load_config()?;
    let mkt_dir = marketplace_dir()?;
    let plugins_dir = mkt_dir.join("plugins");

    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    // Check which plugins exist in Cowork
    let cowork_plugins = find_cowork_plugins_dir();
    let cowork_installed: std::collections::HashSet<String> = cowork_plugins
        .as_ref()
        .and_then(|dir| {
            let ip_path = dir.join("installed_plugins.json");
            if !ip_path.exists() {
                return None;
            }
            let content = fs::read_to_string(&ip_path).ok()?;
            let ip: serde_json::Value = serde_json::from_str(&content).ok()?;
            ip.get("plugins")?.as_object().map(|obj| {
                obj.keys()
                    .filter(|k| k.ends_with(&format!("@{}", MARKETPLACE_NAME)))
                    .map(|k| k.split('@').next().unwrap_or("").to_string())
                    .collect()
            })
        })
        .unwrap_or_default();

    // Check which plugins are enabled in Claude Code
    let code_enabled: std::collections::HashSet<String> = dirs::home_dir()
        .and_then(|h| {
            let settings_path = h.join(".claude").join("settings.json");
            let content = fs::read_to_string(&settings_path).ok()?;
            let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
            settings.get("enabledPlugins")?.as_object().map(|obj| {
                obj.keys()
                    .filter(|k| k.ends_with(&format!("@{}", MARKETPLACE_NAME)) && obj[*k] == true)
                    .map(|k| k.split('@').next().unwrap_or("").to_string())
                    .collect()
            })
        })
        .unwrap_or_default();

    let mut plugins = Vec::new();

    for entry in fs::read_dir(&plugins_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        let manifest_path = entry.path().join(".claude-plugin").join("plugin.json");
        let description = if manifest_path.exists() {
            fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|c| serde_json::from_str::<PluginManifest>(&c).ok())
                .map(|m| m.description)
                .unwrap_or_default()
        } else {
            String::new()
        };

        let (version, installed_at) = config
            .installed_plugins
            .get(&name)
            .map(|e| (e.version.clone(), e.installed_at.clone()))
            .unwrap_or_else(|| ("unknown".to_string(), String::new()));

        let mut targets = Vec::new();
        if code_enabled.contains(&name) {
            targets.push("claude-code".to_string());
        }
        if cowork_installed.contains(&name) {
            targets.push("claude-cowork".to_string());
        }

        plugins.push(InstalledPlugin {
            install_path: entry.path().display().to_string(),
            name,
            version,
            description,
            marketplace: MARKETPLACE_NAME.to_string(),
            installed_at,
            targets,
        });
    }

    Ok(plugins)
}

// --- Helpers ---

#[derive(Debug, Deserialize)]
struct PluginManifest {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    version: String,
    description: String,
}

fn extract_zip(data: &[u8], dest: &Path) -> Result<(), AppError> {
    let reader = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

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

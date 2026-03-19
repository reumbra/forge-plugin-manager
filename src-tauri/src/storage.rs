use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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

#[derive(Debug, Serialize, Clone)]
pub struct CoworkSpace {
    pub id: String,
    pub label: String,
    pub path: String,
    pub is_org: bool,
    pub has_cowork_plugins: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct TargetInfo {
    pub claude_code: bool,
    pub claude_code_path: Option<String>,
    pub cowork_spaces: Vec<CoworkSpace>,
}

pub fn detect_targets() -> TargetInfo {
    let home = dirs::home_dir();

    // Claude Code: ~/.claude/
    let claude_code_path = home.as_ref().map(|h| h.join(".claude"));
    let claude_code = claude_code_path.as_ref().is_some_and(|p| p.exists());

    // Cowork spaces: scan all sessions
    let cowork_spaces = detect_cowork_spaces();

    log::info!(
        "detect_targets: claude_code={}, claude_code_path={:?}, cowork_spaces={}",
        claude_code,
        claude_code_path,
        cowork_spaces.len()
    );
    for space in &cowork_spaces {
        log::info!("  space: id={} label={} is_org={} path={}", space.id, space.label, space.is_org, space.path);
    }

    TargetInfo {
        claude_code,
        claude_code_path: if claude_code { claude_code_path.map(|p| p.display().to_string()) } else { None },
        cowork_spaces,
    }
}

/// Scan all session dirs for cowork spaces (personal + org accounts).
/// Returns deduplicated list of CoworkSpace entries.
fn detect_cowork_spaces() -> Vec<CoworkSpace> {
    let config = match dirs::config_dir() {
        Some(c) => c,
        None => return Vec::new(),
    };
    let claude_dir = config.join("Claude");

    let mut candidates = vec![
        claude_dir.join("claude-code-sessions"),
        claude_dir.join("local-agent-mode-sessions"),
    ];

    // WSL: also check Windows-side Claude data via /mnt/c/
    if cfg!(target_os = "linux") {
        log::info!("detect_cowork_spaces: checking WSL /mnt/c/Users");
        if let Ok(entries) = fs::read_dir("/mnt/c/Users") {
            for entry in entries.flatten() {
                let win_claude = entry.path().join("AppData/Roaming/Claude");
                log::info!("  checking: {}", win_claude.display());
                if win_claude.exists() {
                    candidates.push(win_claude.join("claude-code-sessions"));
                    candidates.push(win_claude.join("local-agent-mode-sessions"));
                    log::info!("  found Claude dir: {}", win_claude.display());
                }
            }
        } else {
            log::warn!("detect_cowork_spaces: cannot read /mnt/c/Users");
        }
    }

    log::info!("detect_cowork_spaces: {} candidate dirs", candidates.len());

    let mut spaces = Vec::new();
    let mut seen_accounts: HashSet<String> = HashSet::new();

    for sessions_dir in &candidates {
        if !sessions_dir.exists() {
            continue;
        }

        let session_entries = match fs::read_dir(sessions_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for session_entry in session_entries.flatten() {
            let session_path = session_entry.path();
            if !session_path.is_dir() || session_entry.file_name() == "skills-plugin" {
                continue;
            }

            let account_entries = match fs::read_dir(&session_path) {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for account_entry in account_entries.flatten() {
                let account_path = account_entry.path();
                if !account_path.is_dir() {
                    continue;
                }

                let account_name = account_entry.file_name().to_string_lossy().to_string();

                // Deduplicate: same account UUID may appear in multiple session dirs
                if seen_accounts.contains(&account_name) {
                    continue;
                }

                let has_cowork_plugins = account_path.join("cowork_plugins").exists();
                let remote_manifest = read_remote_manifest(&account_path);
                let is_org = remote_manifest
                    .as_ref()
                    .map(|m| !m.plugins.is_empty())
                    .unwrap_or(false);

                // Include if it has cowork_plugins OR is an org account
                if !has_cowork_plugins && !is_org {
                    continue;
                }

                // Generate stable ID from path
                let id = {
                    let hash = Sha256::digest(account_path.display().to_string().as_bytes());
                    format!("{:x}", hash)[..8].to_string()
                };

                // Label: for org, use truncated account UUID; personal = "Personal"
                let label = if is_org {
                    format!("Org {}", &account_name[..8.min(account_name.len())])
                } else {
                    "Personal".to_string()
                };

                seen_accounts.insert(account_name);

                spaces.push(CoworkSpace {
                    id,
                    label,
                    path: account_path.display().to_string(),
                    is_org,
                    has_cowork_plugins,
                });
            }
        }
    }

    // Sort: org first, then personal
    spaces.sort_by(|a, b| b.is_org.cmp(&a.is_org));
    spaces
}

/// Remote cowork plugins manifest format (org-synced)
#[derive(Debug, Deserialize)]
struct RemoteManifest {
    #[serde(default)]
    plugins: Vec<RemotePlugin>,
}

#[derive(Debug, Deserialize)]
struct RemotePlugin {
    #[allow(dead_code)]
    name: String,
}

fn read_remote_manifest(account_path: &Path) -> Option<RemoteManifest> {
    let manifest_path = account_path.join("remote_cowork_plugins").join("manifest.json");
    let content = fs::read_to_string(manifest_path).ok()?;
    serde_json::from_str(&content).ok()
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
    owner: serde_json::Value,
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

/// Install a plugin. `target` is either "claude-code" or a cowork space_id.
pub fn install_plugin_from_zip(
    plugin_name: &str,
    version: &str,
    zip_data: &[u8],
    target: &str,
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
    if target == "claude-code" {
        integrate_claude_code(plugin_name)?;
        installed_targets.push("claude-code".to_string());
    } else {
        // target is a cowork space_id — resolve to path
        let spaces = detect_cowork_spaces();
        let space = spaces.iter().find(|s| s.id == target).ok_or_else(|| {
            AppError::CoworkNotFound(format!("Cowork space '{}' not found", target))
        })?;
        let space_path = PathBuf::from(&space.path);
        integrate_cowork_space(plugin_name, version, &description, &plugin_dir, &space_path)?;
        installed_targets.push(format!("cowork:{}:{}", space.id, space.label));
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
            owner: serde_json::json!({"name": "Reumbra", "email": "support@reumbra.dev"}),
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

/// Register marketplace and install plugin in a specific Cowork space.
/// Creates cowork_plugins/ + cowork_settings.json if they don't exist (org accounts).
fn integrate_cowork_space(
    plugin_name: &str,
    version: &str,
    description: &str,
    source_dir: &Path,
    space_path: &Path,
) -> Result<(), AppError> {
    let cowork_dir = space_path.join("cowork_plugins");
    if !cowork_dir.exists() {
        fs::create_dir_all(&cowork_dir)?;
        log::info!("Created cowork_plugins at {}", cowork_dir.display());
    }

    // Ensure cowork_settings.json exists as sibling
    let settings_path = space_path.join("cowork_settings.json");
    if !settings_path.exists() {
        fs::write(&settings_path, "{\"enabledPlugins\":{}}")?;
        log::info!("Created cowork_settings.json at {}", settings_path.display());
    }

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
            owner: serde_json::json!({"name": "Reumbra", "email": "support@reumbra.dev"}),
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

    // 6. Enable plugin in cowork_settings.json
    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    let content = fs::read_to_string(&settings_path)?;
    let mut settings: serde_json::Value = serde_json::from_str(&content)?;
    if settings.get("enabledPlugins").is_none() {
        settings["enabledPlugins"] = serde_json::json!({});
    }
    settings["enabledPlugins"][&plugin_key] = serde_json::Value::Bool(true);
    fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    log::info!("Integrated {} into Cowork space at {}", plugin_name, space_path.display());
    Ok(())
}

/// Uninstall a plugin. `target` is either "claude-code" or a cowork space_id.
pub fn uninstall_plugin(plugin_name: &str, target: &str) -> Result<(), AppError> {
    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);

    if target == "claude-code" {
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

            // Clear cache so Code doesn't load stale copy
            let plugins_dir = home.join(".claude").join("plugins");
            let cache_plugin = plugins_dir.join("cache").join(MARKETPLACE_NAME).join(plugin_name);
            if cache_plugin.exists() {
                let _ = fs::remove_dir_all(&cache_plugin);
            }
        }
    } else {
        // target is a cowork space_id — resolve to path
        let spaces = detect_cowork_spaces();
        if let Some(space) = spaces.iter().find(|s| s.id == target) {
            let cowork_dir = PathBuf::from(&space.path).join("cowork_plugins");
            if cowork_dir.exists() {
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

                // Update marketplace.json in Cowork
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

                // Disable in cowork_settings.json
                let settings_path = PathBuf::from(&space.path).join("cowork_settings.json");
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
    }

    // Only remove from our config if not installed in ANY target
    let still_in_code = is_plugin_in_code(plugin_name);
    let still_in_cowork = is_plugin_in_any_cowork(plugin_name);
    if !still_in_code && !still_in_cowork {
        let mut config = load_config()?;
        config.installed_plugins.remove(plugin_name);
        save_config(&config)?;

        // Now safe to remove from our marketplace dir
        let mkt_dir = marketplace_dir()?;
        let plugin_dir = mkt_dir.join("plugins").join(plugin_name);
        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir)?;
        }
        // Update our marketplace.json
        let manifest_path = mkt_dir.join(".claude-plugin").join("marketplace.json");
        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path)?;
            let mut manifest: MarketplaceManifest = serde_json::from_str(&content)?;
            manifest.plugins.retain(|p| p.name != plugin_name);
            fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;
        }
    }

    Ok(())
}

fn is_plugin_in_code(plugin_name: &str) -> bool {
    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    dirs::home_dir()
        .and_then(|h| {
            let content = fs::read_to_string(h.join(".claude").join("settings.json")).ok()?;
            let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
            settings.get("enabledPlugins")?.get(&plugin_key)
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false)
}

fn is_plugin_in_any_cowork(plugin_name: &str) -> bool {
    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    for space in detect_cowork_spaces() {
        let ip_path = PathBuf::from(&space.path)
            .join("cowork_plugins")
            .join("installed_plugins.json");
        if let Ok(content) = fs::read_to_string(&ip_path) {
            if let Ok(ip) = serde_json::from_str::<serde_json::Value>(&content) {
                if ip.get("plugins").and_then(|p| p.get(&plugin_key)).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

/// List installed plugins from marketplace directory, with per-target status.
/// Cowork targets returned as "cowork:{space_id}:{label}".
pub fn list_installed() -> Result<Vec<InstalledPlugin>, AppError> {
    let config = load_config()?;
    let mkt_dir = marketplace_dir()?;
    let plugins_dir = mkt_dir.join("plugins");

    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }

    // Check which plugins are enabled in Claude Code
    let code_enabled: HashSet<String> = dirs::home_dir()
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

    // Check which plugins exist in each cowork space
    let spaces = detect_cowork_spaces();
    let cowork_space_plugins: Vec<(String, String, HashSet<String>)> = spaces
        .iter()
        .map(|space| {
            let ip_path = PathBuf::from(&space.path)
                .join("cowork_plugins")
                .join("installed_plugins.json");
            let installed: HashSet<String> = fs::read_to_string(&ip_path)
                .ok()
                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                .and_then(|ip| {
                    ip.get("plugins")?.as_object().map(|obj| {
                        obj.keys()
                            .filter(|k| k.ends_with(&format!("@{}", MARKETPLACE_NAME)))
                            .map(|k| k.split('@').next().unwrap_or("").to_string())
                            .collect()
                    })
                })
                .unwrap_or_default();
            (space.id.clone(), space.label.clone(), installed)
        })
        .collect();

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
        for (space_id, space_label, space_plugins) in &cowork_space_plugins {
            if space_plugins.contains(&name) {
                targets.push(format!("cowork:{}:{}", space_id, space_label));
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // --- MarketplaceManifest serde ---

    #[test]
    fn marketplace_manifest_owner_as_object() {
        let json = r#"{
            "name": "reumbra",
            "owner": {"name": "Reumbra", "email": "support@reumbra.dev"},
            "plugins": []
        }"#;
        let manifest: MarketplaceManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "reumbra");
        assert_eq!(manifest.owner["name"], "Reumbra");
        assert_eq!(manifest.owner["email"], "support@reumbra.dev");
    }

    #[test]
    fn marketplace_manifest_owner_as_string() {
        let json = r#"{
            "name": "reumbra",
            "owner": "Reumbra",
            "plugins": []
        }"#;
        let manifest: MarketplaceManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.owner, "Reumbra");
    }

    #[test]
    fn marketplace_manifest_roundtrip_with_plugins() {
        let manifest = MarketplaceManifest {
            name: "reumbra".to_string(),
            owner: serde_json::json!({"name": "Reumbra", "email": "support@reumbra.dev"}),
            plugins: vec![
                MarketplacePlugin {
                    name: "forge-core".to_string(),
                    source: "./plugins/forge-core".to_string(),
                    description: Some("Core plugin".to_string()),
                    version: Some("6.0.0".to_string()),
                },
            ],
        };

        let serialized = serde_json::to_string_pretty(&manifest).unwrap();
        let deserialized: MarketplaceManifest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.name, "reumbra");
        assert_eq!(deserialized.plugins.len(), 1);
        assert_eq!(deserialized.plugins[0].name, "forge-core");
        assert_eq!(deserialized.plugins[0].version, Some("6.0.0".to_string()));
    }

    #[test]
    fn marketplace_manifest_default_empty_plugins() {
        let json = r#"{"name": "test", "owner": "Test"}"#;
        let manifest: MarketplaceManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.plugins.is_empty());
    }

    // --- ForgeConfig serde ---

    #[test]
    fn forge_config_default_is_empty() {
        let config = ForgeConfig::default();
        assert!(config.license_key.is_none());
        assert!(config.machine_id.is_none());
        assert!(config.plan.is_none());
        assert!(config.installed_plugins.is_empty());
    }

    #[test]
    fn forge_config_roundtrip() {
        let mut config = ForgeConfig::default();
        config.license_key = Some("FRG-ABCD-EFGH-IJKL".to_string());
        config.plan = Some("pro".to_string());
        config.installed_plugins.insert(
            "forge-core".to_string(),
            InstalledPluginEntry {
                version: "6.0.0".to_string(),
                installed_at: "2026-03-08T00:00:00Z".to_string(),
            },
        );

        let json = serde_json::to_string(&config).unwrap();
        let restored: ForgeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.license_key.unwrap(), "FRG-ABCD-EFGH-IJKL");
        assert_eq!(restored.plan.unwrap(), "pro");
        assert!(restored.installed_plugins.contains_key("forge-core"));
        assert_eq!(restored.installed_plugins["forge-core"].version, "6.0.0");
    }

    #[test]
    fn forge_config_skips_none_fields() {
        let config = ForgeConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("license_key"));
        assert!(!json.contains("machine_id"));
    }

    // --- Plugin key format ---

    #[test]
    fn plugin_key_format_matches_convention() {
        let name = "forge-core";
        let key = format!("{}@{}", name, MARKETPLACE_NAME);
        assert_eq!(key, "forge-core@reumbra");
    }

    // --- is_plugin_in_code with temp filesystem ---

    #[test]
    fn is_plugin_in_code_logic_with_settings_json() {
        // This test verifies the JSON parsing logic used by is_plugin_in_code.
        // We can't override dirs::home_dir(), so we test the JSON logic directly.
        let settings = serde_json::json!({
            "enabledPlugins": {
                "forge-core@reumbra": true,
                "forge-qa@reumbra": false
            }
        });

        let plugin_key = format!("{}@{}", "forge-core", MARKETPLACE_NAME);
        let is_enabled = settings
            .get("enabledPlugins")
            .and_then(|ep| ep.get(&plugin_key))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(is_enabled, "forge-core should be enabled");

        let plugin_key_qa = format!("{}@{}", "forge-qa", MARKETPLACE_NAME);
        let is_qa_enabled = settings
            .get("enabledPlugins")
            .and_then(|ep| ep.get(&plugin_key_qa))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(!is_qa_enabled, "forge-qa should be disabled (false)");

        let plugin_key_missing = format!("{}@{}", "nonexistent", MARKETPLACE_NAME);
        let is_missing = settings
            .get("enabledPlugins")
            .and_then(|ep| ep.get(&plugin_key_missing))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(!is_missing, "nonexistent plugin should not be found");
    }

    // --- is_plugin_in_cowork logic ---

    #[test]
    fn is_plugin_in_cowork_logic_with_installed_plugins_json() {
        let ip = serde_json::json!({
            "version": 2,
            "plugins": {
                "forge-core@reumbra": [{
                    "scope": "user",
                    "installPath": "mnt/.claude/cowork_plugins/cache/reumbra/forge-core/6.0.0",
                    "version": "6.0.0"
                }]
            }
        });

        let plugin_key = format!("{}@{}", "forge-core", MARKETPLACE_NAME);
        let found = ip.get("plugins")
            .and_then(|p| p.get(&plugin_key))
            .is_some();
        assert!(found, "forge-core should be in cowork");

        let missing_key = format!("{}@{}", "nonexistent", MARKETPLACE_NAME);
        let not_found = ip.get("plugins")
            .and_then(|p| p.get(&missing_key))
            .is_some();
        assert!(!not_found, "nonexistent should not be in cowork");
    }

    // --- Cowork installed_plugins.json structure ---

    #[test]
    fn cowork_installed_plugins_structure() {
        // Verifies the exact structure we write to installed_plugins.json
        let plugin_name = "forge-core";
        let version = "6.2.0";
        let mut ip = serde_json::json!({ "version": 2, "plugins": {} });

        let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
        let cache_rel_path = format!(
            "mnt/.claude/cowork_plugins/cache/{}/{}/{}",
            MARKETPLACE_NAME, plugin_name, version
        );
        ip["plugins"][&plugin_key] = serde_json::json!([{
            "scope": "user",
            "installPath": cache_rel_path,
            "version": version,
            "installedAt": "2026-03-08T00:00:00Z",
            "lastUpdated": "2026-03-08T00:00:00Z"
        }]);

        // Verify structure
        let plugins = ip["plugins"].as_object().unwrap();
        assert_eq!(plugins.len(), 1);
        assert!(plugins.contains_key("forge-core@reumbra"));

        let entries = plugins["forge-core@reumbra"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["scope"], "user");
        assert_eq!(
            entries[0]["installPath"],
            "mnt/.claude/cowork_plugins/cache/reumbra/forge-core/6.2.0"
        );
    }

    // --- known_marketplaces.json structure ---

    #[test]
    fn known_marketplaces_code_structure() {
        let mkt_path = "/home/user/.config/forge-devkit/marketplace";
        let mut km = serde_json::json!({});
        km[MARKETPLACE_NAME] = serde_json::json!({
            "source": { "source": "directory", "path": mkt_path },
            "installLocation": mkt_path,
            "lastUpdated": "2026-03-08T00:00:00Z"
        });

        assert_eq!(km["reumbra"]["source"]["source"], "directory");
        assert_eq!(km["reumbra"]["source"]["path"], mkt_path);
        assert_eq!(km["reumbra"]["installLocation"], mkt_path);
    }

    #[test]
    fn known_marketplaces_cowork_uses_relative_paths() {
        let mkt_rel_path = format!("mnt/.claude/cowork_plugins/marketplaces/{}", MARKETPLACE_NAME);
        let mut km = serde_json::json!({});
        km[MARKETPLACE_NAME] = serde_json::json!({
            "source": { "source": "directory", "path": &mkt_rel_path },
            "installLocation": &mkt_rel_path,
            "lastUpdated": "2026-03-08T00:00:00Z"
        });

        let path = km["reumbra"]["source"]["path"].as_str().unwrap();
        assert!(path.starts_with("mnt/"), "Cowork paths must start with mnt/");
        assert!(!path.starts_with("/"), "Cowork paths must be relative");
    }

    // --- Filesystem-based tests with tempdir ---

    #[test]
    fn copy_dir_recursive_works() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");

        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("file.txt"), "hello").unwrap();
        fs::write(src.join("subdir").join("nested.txt"), "world").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert!(dst.join("file.txt").exists());
        assert!(dst.join("subdir").join("nested.txt").exists());
        assert_eq!(fs::read_to_string(dst.join("file.txt")).unwrap(), "hello");
        assert_eq!(
            fs::read_to_string(dst.join("subdir").join("nested.txt")).unwrap(),
            "world"
        );
    }

    #[test]
    fn marketplace_manifest_file_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let manifest_path = tmp.path().join("marketplace.json");

        let manifest = MarketplaceManifest {
            name: MARKETPLACE_NAME.to_string(),
            owner: serde_json::json!({"name": "Reumbra", "email": "support@reumbra.dev"}),
            plugins: vec![
                MarketplacePlugin {
                    name: "forge-core".to_string(),
                    source: "./plugins/forge-core".to_string(),
                    description: Some("Core plugin".to_string()),
                    version: Some("6.0.0".to_string()),
                },
                MarketplacePlugin {
                    name: "forge-qa".to_string(),
                    source: "./plugins/forge-qa".to_string(),
                    description: None,
                    version: None,
                },
            ],
        };

        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();

        let content = fs::read_to_string(&manifest_path).unwrap();
        let restored: MarketplaceManifest = serde_json::from_str(&content).unwrap();

        assert_eq!(restored.plugins.len(), 2);
        assert_eq!(restored.plugins[0].name, "forge-core");
        assert!(restored.plugins[1].description.is_none());
    }

    #[test]
    fn uninstall_removes_plugin_from_manifest() {
        // Simulate the manifest update logic from uninstall_plugin
        let mut manifest = MarketplaceManifest {
            name: MARKETPLACE_NAME.to_string(),
            owner: serde_json::json!("Reumbra"),
            plugins: vec![
                MarketplacePlugin {
                    name: "forge-core".to_string(),
                    source: "./plugins/forge-core".to_string(),
                    description: None,
                    version: Some("6.0.0".to_string()),
                },
                MarketplacePlugin {
                    name: "forge-qa".to_string(),
                    source: "./plugins/forge-qa".to_string(),
                    description: None,
                    version: Some("3.0.0".to_string()),
                },
            ],
        };

        manifest.plugins.retain(|p| p.name != "forge-core");
        assert_eq!(manifest.plugins.len(), 1);
        assert_eq!(manifest.plugins[0].name, "forge-qa");
    }

    // --- PluginManifest (plugin.json) ---

    #[test]
    fn plugin_manifest_parses() {
        let json = r#"{
            "name": "forge-core",
            "version": "6.2.0",
            "description": "Core development pipeline for Claude Code"
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.description, "Core development pipeline for Claude Code");
    }

    // --- InstalledPlugin targets ---

    #[test]
    fn installed_plugin_both_targets() {
        let plugin = InstalledPlugin {
            name: "forge-core".to_string(),
            version: "6.0.0".to_string(),
            description: "test".to_string(),
            marketplace: MARKETPLACE_NAME.to_string(),
            installed_at: "2026-03-08T00:00:00Z".to_string(),
            install_path: "/tmp/test".to_string(),
            targets: vec!["claude-code".to_string(), "claude-cowork".to_string()],
        };

        assert!(plugin.targets.contains(&"claude-code".to_string()));
        assert!(plugin.targets.contains(&"claude-cowork".to_string()));
        assert_eq!(plugin.targets.len(), 2);
    }

    #[test]
    fn installed_plugin_single_target() {
        let plugin = InstalledPlugin {
            name: "forge-core".to_string(),
            version: "6.0.0".to_string(),
            description: "test".to_string(),
            marketplace: MARKETPLACE_NAME.to_string(),
            installed_at: String::new(),
            install_path: String::new(),
            targets: vec!["claude-cowork".to_string()],
        };

        assert!(!plugin.targets.contains(&"claude-code".to_string()));
        assert!(plugin.targets.contains(&"claude-cowork".to_string()));
    }
}

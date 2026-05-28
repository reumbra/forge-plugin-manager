use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
        .ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot determine config directory",
            ))
        })
}

/// Get the forge-devkit cache directory.
/// Windows: %LOCALAPPDATA%/forge-devkit/cache
/// macOS:   ~/Library/Caches/forge-devkit
/// Linux:   ~/.cache/forge-devkit
#[allow(dead_code)]
pub fn cache_dir() -> Result<PathBuf, AppError> {
    dirs::cache_dir()
        .map(|d| d.join(APP_DIR_NAME))
        .ok_or_else(|| {
            AppError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cannot determine cache directory",
            ))
        })
}

/// Path to the marketplace directory
pub fn marketplace_dir() -> Result<PathBuf, AppError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Other("Cannot determine home directory".into()))?;
    Ok(home
        .join(".claude")
        .join("plugins")
        .join("marketplaces")
        .join(MARKETPLACE_NAME))
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
    let home = dirs::home_dir().ok_or_else(|| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Cannot determine home directory",
        ))
    })?;

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
        log::info!(
            "  space: id={} label={} is_org={} path={}",
            space.id,
            space.label,
            space.is_org,
            space.path
        );
    }

    TargetInfo {
        claude_code,
        claude_code_path: if claude_code {
            claude_code_path.map(|p| p.display().to_string())
        } else {
            None
        },
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

    let candidates = vec![
        claude_dir.join("claude-code-sessions"),
        claude_dir.join("local-agent-mode-sessions"),
    ];

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
    let manifest_path = account_path
        .join("remote_cowork_plugins")
        .join("manifest.json");
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
    migrate_legacy_marketplace_if_present()?;

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
        #[allow(deprecated)]
        {
            integrate_cowork_space(plugin_name, version, &description, &plugin_dir, &space_path)?;
        }
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

    // Claude Desktop requires owner to be an object; normalize API/string data on every write.
    let marketplace_owner = serde_json::json!({"name": "Reumbra", "email": "support@reumbra.dev"});
    let mut manifest = if manifest_path.exists() {
        let content = fs::read_to_string(&manifest_path)?;
        serde_json::from_str(&content)?
    } else {
        MarketplaceManifest {
            name: MARKETPLACE_NAME.to_string(),
            owner: marketplace_owner.clone(),
            plugins: Vec::new(),
        }
    };
    manifest.owner = marketplace_owner;

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

fn migrate_legacy_marketplace_if_present() -> Result<(), AppError> {
    let Some(home) = dirs::home_dir() else {
        return Ok(());
    };

    let legacy_mkt_dir = config_dir()?.join("marketplace");
    let canonical_mkt_dir = marketplace_dir()?;

    // Step 1: copy marketplace files (idempotent — skip if canonical already exists).
    if !canonical_mkt_dir.exists() && legacy_mkt_dir.exists() {
        if let Some(parent) = canonical_mkt_dir.parent() {
            fs::create_dir_all(parent)?;
        }

        let legacy_plugins_dir = legacy_mkt_dir.join("plugins");
        if legacy_plugins_dir.exists() {
            let canonical_plugins_dir = canonical_mkt_dir.join("plugins");
            copy_dir_recursive(&legacy_plugins_dir, &canonical_plugins_dir)?;
        }

        let legacy_manifest = legacy_mkt_dir
            .join(".claude-plugin")
            .join("marketplace.json");
        if legacy_manifest.exists() {
            let canonical_manifest = canonical_mkt_dir
                .join(".claude-plugin")
                .join("marketplace.json");
            if let Some(parent) = canonical_manifest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&legacy_manifest, &canonical_manifest)?;
        }

        log::info!(
            "Migrated marketplace from legacy {} to canonical {}",
            legacy_mkt_dir.display(),
            canonical_mkt_dir.display()
        );
    }

    // Step 2: rewrite legacy registry paths to canonical (independently idempotent).
    // Customers upgrading from v0.5.3 have known_marketplaces.json + installed_plugins.json
    // entries pointing into legacy_mkt_dir. Without this rewrite they stay broken
    // (out-of-bounds) until each plugin is re-installed individually.
    rewrite_legacy_registry_paths_if_needed(&home, &legacy_mkt_dir, &canonical_mkt_dir)?;

    Ok(())
}

/// Rewrite `~/.claude/plugins/known_marketplaces.json` reumbra entry and any
/// `installed_plugins.json` `installPath` whose value starts with the legacy
/// marketplace dir. Idempotent: only writes when a legacy path is actually found.
fn rewrite_legacy_registry_paths_if_needed(
    home: &Path,
    legacy_mkt_dir: &Path,
    canonical_mkt_dir: &Path,
) -> Result<(), AppError> {
    let plugins_dir = home.join(".claude").join("plugins");
    if !plugins_dir.exists() {
        return Ok(());
    }

    let legacy_mkt_str = legacy_mkt_dir.display().to_string();
    let canonical_mkt_str = canonical_mkt_dir.display().to_string();
    let legacy_plugins_str = legacy_mkt_dir.join("plugins").display().to_string();
    let canonical_plugins_str = canonical_mkt_dir.join("plugins").display().to_string();

    // 1. known_marketplaces.json
    let km_path = plugins_dir.join("known_marketplaces.json");
    if km_path.exists() {
        let content = fs::read_to_string(&km_path)?;
        if let Ok(mut km) = serde_json::from_str::<Value>(&content) {
            let mut changed = false;
            if let Some(entry) = km.get_mut(MARKETPLACE_NAME) {
                if let Some(loc) = entry.get("installLocation").and_then(|v| v.as_str()) {
                    if loc == legacy_mkt_str {
                        entry["installLocation"] = Value::String(canonical_mkt_str.clone());
                        changed = true;
                    }
                }
                if let Some(src_path) = entry
                    .get("source")
                    .and_then(|s| s.get("path"))
                    .and_then(|v| v.as_str())
                {
                    if src_path == legacy_mkt_str {
                        entry["source"]["path"] = Value::String(canonical_mkt_str.clone());
                        changed = true;
                    }
                }
            }
            if changed {
                fs::write(&km_path, serde_json::to_string_pretty(&km)?)?;
                log::info!(
                    "Rewrote known_marketplaces.json {} entry: {} -> {}",
                    MARKETPLACE_NAME,
                    legacy_mkt_str,
                    canonical_mkt_str
                );
            }
        }
    }

    // 2. installed_plugins.json — rewrite installPath for every *@reumbra entry
    //    whose path still starts with the legacy plugins dir.
    let ip_path = plugins_dir.join("installed_plugins.json");
    if ip_path.exists() {
        let content = fs::read_to_string(&ip_path)?;
        if let Ok(mut ip) = serde_json::from_str::<Value>(&content) {
            let suffix = format!("@{}", MARKETPLACE_NAME);
            let mut changed = false;
            if let Some(plugins) = ip.get_mut("plugins").and_then(|v| v.as_object_mut()) {
                for (key, entries) in plugins.iter_mut() {
                    if !key.ends_with(&suffix) {
                        continue;
                    }
                    if let Some(arr) = entries.as_array_mut() {
                        for inst in arr.iter_mut() {
                            if let Some(p) = inst.get("installPath").and_then(|v| v.as_str()) {
                                if p.starts_with(&legacy_plugins_str) {
                                    let new_path =
                                        p.replacen(&legacy_plugins_str, &canonical_plugins_str, 1);
                                    inst["installPath"] = Value::String(new_path);
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
            if changed {
                fs::write(&ip_path, serde_json::to_string_pretty(&ip)?)?;
                log::info!(
                    "Rewrote installed_plugins.json installPath entries: {} -> {}",
                    legacy_plugins_str,
                    canonical_plugins_str
                );
            }
        }
    }

    Ok(())
}

/// Register marketplace and enable plugin in Claude Code
fn integrate_claude_code(plugin_name: &str) -> Result<(), AppError> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Ok(()), // Can't integrate without home dir
    };

    migrate_legacy_marketplace_if_present()?;

    let claude_dir = home.join(".claude");
    if !claude_dir.exists() {
        log::warn!("Claude Code not detected, skipping integration");
        return Ok(());
    }

    let plugins_dir = claude_dir.join("plugins");
    fs::create_dir_all(&plugins_dir)?;
    let now = Utc::now().to_rfc3339();

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
        "lastUpdated": &now
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
    let cache_plugin = plugins_dir
        .join("cache")
        .join(MARKETPLACE_NAME)
        .join(plugin_name);
    if cache_plugin.exists() {
        let _ = fs::remove_dir_all(&cache_plugin);
        log::info!("Cleared stale cache for {}", plugin_name);
    }

    let active_copy = plugins_dir.join(plugin_name);
    if active_copy.exists() {
        let _ = fs::remove_dir_all(&active_copy);
        log::info!("Cleared stale active copy for {}", plugin_name);
    }

    // 4. Write installed_plugins.json with the real extracted plugin path.
    write_claude_code_installed_plugin(&plugins_dir, plugin_name, &now)?;

    Ok(())
}

fn write_claude_code_installed_plugin(
    plugins_dir: &Path,
    plugin_name: &str,
    timestamp: &str,
) -> Result<(), AppError> {
    let plugin_dir = marketplace_dir()?.join("plugins").join(plugin_name);
    let manifest_path = plugin_dir.join(".claude-plugin").join("plugin.json");
    let manifest: PluginManifest = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    let install_path = plugin_dir
        .canonicalize()
        .unwrap_or_else(|_| plugin_dir.clone())
        .display()
        .to_string();

    let ip_path = plugins_dir.join("installed_plugins.json");
    let mut ip: Value = if ip_path.exists() {
        let content = fs::read_to_string(&ip_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({ "version": 2, "plugins": {} })
    };

    if ip.get("version").is_none() {
        ip["version"] = serde_json::json!(2);
    }
    if !matches!(ip.get("plugins"), Some(Value::Object(_))) {
        ip["plugins"] = serde_json::json!({});
    }

    let plugin_key = format!("{}@{}", plugin_name, MARKETPLACE_NAME);
    ip["plugins"][&plugin_key] = serde_json::json!([{
        "scope": "user",
        "installPath": install_path,
        "version": manifest.version,
        "installedAt": timestamp,
        "lastUpdated": timestamp
    }]);
    fs::write(&ip_path, serde_json::to_string_pretty(&ip)?)?;
    Ok(())
}

fn remove_claude_code_installed_plugin(
    plugins_dir: &Path,
    plugin_key: &str,
) -> Result<(), AppError> {
    let ip_path = plugins_dir.join("installed_plugins.json");
    if !ip_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&ip_path)?;
    let mut ip: Value = serde_json::from_str(&content)?;
    if let Some(plugins) = ip
        .get_mut("plugins")
        .and_then(|plugins| plugins.as_object_mut())
    {
        plugins.remove(plugin_key);
    }
    fs::write(&ip_path, serde_json::to_string_pretty(&ip)?)?;
    Ok(())
}

/// Register marketplace and install plugin in a specific Cowork space.
/// Creates cowork_plugins/ + cowork_settings.json if they don't exist (org accounts).
#[deprecated(
    note = "Legacy cowork_plugins/ store no longer authoritative — use integrate_claude_code instead. Slated for removal in v0.7.0."
)]
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
        log::info!(
            "Created cowork_settings.json at {}",
            settings_path.display()
        );
    }

    // 1. Copy plugin to marketplaces/reumbra/{plugin_name}/
    let mkt_plugin_dir = cowork_dir
        .join("marketplaces")
        .join(MARKETPLACE_NAME)
        .join(plugin_name);
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

    let mkt_rel_path = format!(
        "mnt/.claude/cowork_plugins/marketplaces/{}",
        MARKETPLACE_NAME
    );
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

    log::info!(
        "Integrated {} into Cowork space at {}",
        plugin_name,
        space_path.display()
    );
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
            let cache_plugin = plugins_dir
                .join("cache")
                .join(MARKETPLACE_NAME)
                .join(plugin_name);
            if cache_plugin.exists() {
                let _ = fs::remove_dir_all(&cache_plugin);
            }
            remove_claude_code_installed_plugin(&plugins_dir, &plugin_key)?;
        }
    } else {
        // target is a cowork space_id — resolve to path
        let spaces = detect_cowork_spaces();
        if let Some(space) = spaces.iter().find(|s| s.id == target) {
            let cowork_dir = PathBuf::from(&space.path).join("cowork_plugins");
            if cowork_dir.exists() {
                // Remove from marketplaces/reumbra/{plugin}
                let mkt_plugin_dir = cowork_dir
                    .join("marketplaces")
                    .join(MARKETPLACE_NAME)
                    .join(plugin_name);
                if mkt_plugin_dir.exists() {
                    fs::remove_dir_all(&mkt_plugin_dir)?;
                }

                // Remove from cache/reumbra/{plugin}
                let cache_plugin_dir = cowork_dir
                    .join("cache")
                    .join(MARKETPLACE_NAME)
                    .join(plugin_name);
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
            settings
                .get("enabledPlugins")?
                .get(&plugin_key)
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
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use tempfile::TempDir;

    static CLAUDE_CODE_TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn set(vars: Vec<(&'static str, OsString)>) -> Self {
            let saved = vars
                .iter()
                .map(|(key, _)| (*key, std::env::var_os(key)))
                .collect();
            for (key, value) in vars {
                std::env::set_var(key, value);
            }
            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    fn with_temp_app_env<T>(run: impl FnOnce(&Path) -> T) -> T {
        let _lock = CLAUDE_CODE_TEST_ENV_LOCK.lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let config = tmp.path().join("config");
        let local = tmp.path().join("local");

        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(&config).unwrap();
        fs::create_dir_all(&local).unwrap();

        let _env = EnvGuard::set(vec![
            ("HOME", home.as_os_str().to_os_string()),
            ("USERPROFILE", home.as_os_str().to_os_string()),
            ("XDG_CONFIG_HOME", config.as_os_str().to_os_string()),
            ("APPDATA", config.as_os_str().to_os_string()),
            ("LOCALAPPDATA", local.as_os_str().to_os_string()),
        ]);

        run(&home)
    }

    fn create_marketplace_plugin(plugin_name: &str, version: &str) -> PathBuf {
        let plugin_dir = marketplace_dir().unwrap().join("plugins").join(plugin_name);
        create_plugin_at(&plugin_dir, plugin_name, version)
    }

    fn create_plugin_at(plugin_dir: &Path, plugin_name: &str, version: &str) -> PathBuf {
        fs::create_dir_all(plugin_dir.join(".claude-plugin")).unwrap();
        fs::create_dir_all(plugin_dir.join("skills")).unwrap();
        fs::write(plugin_dir.join("skills").join("README.md"), "fixture skill").unwrap();

        let manifest = serde_json::json!({
            "name": plugin_name,
            "version": version,
            "description": "Fixture plugin"
        });
        fs::write(
            plugin_dir.join(".claude-plugin").join("plugin.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        plugin_dir.to_path_buf()
    }

    fn read_json(path: impl AsRef<Path>) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

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
            plugins: vec![MarketplacePlugin {
                name: "forge-core".to_string(),
                source: "./plugins/forge-core".to_string(),
                description: Some("Core plugin".to_string()),
                version: Some("6.0.0".to_string()),
            }],
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

    // --- Claude Code integration regressions ---

    #[test]
    fn claude_code_uses_canonical_marketplace_dir() {
        with_temp_app_env(|home| {
            let mkt_dir = marketplace_dir().unwrap();
            let expected = home
                .join(".claude")
                .join("plugins")
                .join("marketplaces")
                .join("reumbra");

            assert_eq!(mkt_dir, expected);
            assert!(mkt_dir
                .display()
                .to_string()
                .contains(".claude/plugins/marketplaces/reumbra"));
        });
    }

    #[test]
    fn claude_code_marketplace_owner_is_object() {
        with_temp_app_env(|_| {
            let manifest_path = marketplace_dir()
                .unwrap()
                .join(".claude-plugin")
                .join("marketplace.json");
            fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
            fs::write(
                &manifest_path,
                serde_json::json!({
                    "name": MARKETPLACE_NAME,
                    "owner": "Reumbra",
                    "plugins": []
                })
                .to_string(),
            )
            .unwrap();

            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();

            let manifest = read_json(&manifest_path);
            assert!(manifest["owner"].is_object());
            assert_eq!(manifest["owner"]["name"], "Reumbra");
            assert_eq!(manifest["owner"]["email"], "support@reumbra.dev");
        });
    }

    #[test]
    fn claude_code_install_path_points_to_real_dir() {
        with_temp_app_env(|home| {
            let plugin_dir = create_marketplace_plugin("forge-core", "6.0.0");
            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();

            integrate_claude_code("forge-core").unwrap();

            let installed_plugins = read_json(
                home.join(".claude")
                    .join("plugins")
                    .join("installed_plugins.json"),
            );
            let entry = &installed_plugins["plugins"]["forge-core@reumbra"][0];
            let install_path = PathBuf::from(entry["installPath"].as_str().unwrap());

            assert!(install_path.is_absolute());
            assert_eq!(install_path, plugin_dir.canonicalize().unwrap());
            assert!(install_path
                .display()
                .to_string()
                .contains(".claude/plugins/marketplaces/reumbra/plugins/forge-core"));
            assert!(install_path.exists());
            assert!(install_path
                .join(".claude-plugin")
                .join("plugin.json")
                .exists());
            assert!(["skills", "agents", "commands"]
                .iter()
                .any(|dir| install_path.join(dir).exists()));
            assert_eq!(entry["version"], "6.0.0");
        });
    }

    #[test]
    fn claude_code_install_path_is_inside_claude_home() {
        with_temp_app_env(|home| {
            create_marketplace_plugin("forge-core", "6.0.0");
            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();

            integrate_claude_code("forge-core").unwrap();

            let installed_plugins = read_json(
                home.join(".claude")
                    .join("plugins")
                    .join("installed_plugins.json"),
            );
            let entry = &installed_plugins["plugins"]["forge-core@reumbra"][0];
            let install_path = entry["installPath"].as_str().unwrap();

            assert!(install_path.contains(".claude/plugins"));
            assert!(install_path.contains(".claude/plugins/marketplaces/reumbra"));
        });
    }

    #[test]
    fn claude_code_known_marketplaces_points_to_canonical() {
        with_temp_app_env(|home| {
            create_marketplace_plugin("forge-core", "6.0.0");
            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();

            integrate_claude_code("forge-core").unwrap();

            let known_marketplaces = read_json(
                home.join(".claude")
                    .join("plugins")
                    .join("known_marketplaces.json"),
            );
            let marketplace_path = known_marketplaces["reumbra"]["source"]["path"]
                .as_str()
                .unwrap();

            assert!(marketplace_path.contains(".claude/plugins/marketplaces/reumbra"));
            assert_eq!(
                marketplace_path,
                marketplace_dir().unwrap().display().to_string()
            );
        });
    }

    #[test]
    fn migrate_legacy_marketplace_copies_files() {
        with_temp_app_env(|_| {
            let legacy_mkt_dir = config_dir().unwrap().join("marketplace");
            let legacy_plugin_dir = legacy_mkt_dir.join("plugins").join("forge-core");
            create_plugin_at(&legacy_plugin_dir, "forge-core", "6.0.0");
            let legacy_manifest = legacy_mkt_dir
                .join(".claude-plugin")
                .join("marketplace.json");
            fs::create_dir_all(legacy_manifest.parent().unwrap()).unwrap();
            fs::write(
                &legacy_manifest,
                serde_json::json!({
                    "name": MARKETPLACE_NAME,
                    "owner": {"name": "Reumbra", "email": "support@reumbra.dev"},
                    "plugins": [{
                        "name": "forge-core",
                        "source": "./plugins/forge-core",
                        "description": "Core plugin",
                        "version": "6.0.0"
                    }]
                })
                .to_string(),
            )
            .unwrap();

            integrate_claude_code("forge-core").unwrap();

            let canonical_mkt_dir = marketplace_dir().unwrap();
            assert!(legacy_plugin_dir.exists());
            assert!(canonical_mkt_dir
                .join("plugins")
                .join("forge-core")
                .join(".claude-plugin")
                .join("plugin.json")
                .exists());
            assert!(canonical_mkt_dir
                .join("plugins")
                .join("forge-core")
                .join("skills")
                .join("README.md")
                .exists());
            assert!(canonical_mkt_dir
                .join(".claude-plugin")
                .join("marketplace.json")
                .exists());
        });
    }

    #[test]
    fn migrate_legacy_marketplace_idempotent() {
        with_temp_app_env(|_| {
            let legacy_plugin_dir = config_dir()
                .unwrap()
                .join("marketplace")
                .join("plugins")
                .join("forge-core");
            create_plugin_at(&legacy_plugin_dir, "forge-core", "5.0.0");
            fs::write(
                legacy_plugin_dir.join("skills").join("README.md"),
                "legacy skill",
            )
            .unwrap();

            let canonical_plugin_dir = marketplace_dir()
                .unwrap()
                .join("plugins")
                .join("forge-core");
            create_plugin_at(&canonical_plugin_dir, "forge-core", "6.0.0");
            fs::write(
                canonical_plugin_dir.join("skills").join("README.md"),
                "canonical skill",
            )
            .unwrap();

            integrate_claude_code("forge-core").unwrap();

            let skill_readme =
                fs::read_to_string(canonical_plugin_dir.join("skills").join("README.md")).unwrap();
            let manifest = read_json(
                canonical_plugin_dir
                    .join(".claude-plugin")
                    .join("plugin.json"),
            );

            assert_eq!(skill_readme, "canonical skill");
            assert_eq!(manifest["version"], "6.0.0");
        });
    }

    #[test]
    fn migrate_legacy_marketplace_rewrites_registry_entries() {
        // v0.5.3 customer scenario: legacy marketplace dir exists, registry files
        // already reference the legacy paths. After migration the registry must
        // point to the canonical location, otherwise installed plugins remain
        // out-of-bounds for Claude Code's LocalPluginsReader.
        with_temp_app_env(|home| {
            // Seed legacy marketplace with one plugin
            let legacy_mkt_dir = config_dir().unwrap().join("marketplace");
            let legacy_plugin_dir = legacy_mkt_dir.join("plugins").join("forge-core");
            create_plugin_at(&legacy_plugin_dir, "forge-core", "11.1.0");
            let legacy_manifest = legacy_mkt_dir
                .join(".claude-plugin")
                .join("marketplace.json");
            fs::create_dir_all(legacy_manifest.parent().unwrap()).unwrap();
            fs::write(
                &legacy_manifest,
                serde_json::json!({
                    "name": MARKETPLACE_NAME,
                    "owner": {"name": "Reumbra", "email": "support@reumbra.dev"},
                    "plugins": [{"name": "forge-core", "source": "./plugins/forge-core", "version": "11.1.0"}]
                })
                .to_string(),
            )
            .unwrap();

            // Pre-existing registry (v0.5.3-era) pointing to legacy paths
            let plugins_dir = home.join(".claude").join("plugins");
            fs::create_dir_all(&plugins_dir).unwrap();
            let legacy_mkt_str = legacy_mkt_dir.display().to_string();
            let legacy_plugin_str = legacy_plugin_dir.display().to_string();
            fs::write(
                plugins_dir.join("known_marketplaces.json"),
                serde_json::json!({
                    MARKETPLACE_NAME: {
                        "installLocation": legacy_mkt_str,
                        "source": {"source": "directory", "path": legacy_mkt_str},
                        "lastUpdated": "2026-05-27T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .unwrap();
            fs::write(
                plugins_dir.join("installed_plugins.json"),
                serde_json::json!({
                    "version": 2,
                    "plugins": {
                        "forge-core@reumbra": [{
                            "scope": "user",
                            "installPath": legacy_plugin_str,
                            "version": "11.1.0",
                            "installedAt": "2026-05-27T00:00:00Z",
                            "lastUpdated": "2026-05-27T00:00:00Z"
                        }]
                    }
                })
                .to_string(),
            )
            .unwrap();

            // Install a SECOND plugin via v0.6.0 — triggers the migration helper
            let second = marketplace_dir()
                .unwrap()
                .join("plugins")
                .join("forge-product");
            create_plugin_at(&second, "forge-product", "4.6.0");
            // The marketplace.json must list forge-product so update_marketplace_manifest()
            // doesn't fail; reuse the canonical helper to add it.
            update_marketplace_manifest("forge-product", "4.6.0", "Product plugin").unwrap();
            integrate_claude_code("forge-product").unwrap();

            // Registry must now point to canonical for the existing forge-core entry
            let canonical_mkt = marketplace_dir().unwrap().display().to_string();
            let canonical_plugin = marketplace_dir()
                .unwrap()
                .join("plugins")
                .join("forge-core")
                .display()
                .to_string();

            let km = read_json(plugins_dir.join("known_marketplaces.json"));
            assert_eq!(km[MARKETPLACE_NAME]["installLocation"], canonical_mkt);
            assert_eq!(km[MARKETPLACE_NAME]["source"]["path"], canonical_mkt);

            let ip = read_json(plugins_dir.join("installed_plugins.json"));
            assert_eq!(
                ip["plugins"]["forge-core@reumbra"][0]["installPath"],
                canonical_plugin
            );
        });
    }

    #[test]
    fn migrate_legacy_marketplace_registry_rewrite_idempotent() {
        // Already-migrated customer: registry has canonical paths.
        // Re-running migration must not touch them and must not corrupt the JSON
        // structure (no unintended rewrites).
        with_temp_app_env(|home| {
            create_marketplace_plugin("forge-core", "11.1.0");
            update_marketplace_manifest("forge-core", "11.1.0", "Core plugin").unwrap();

            let plugins_dir = home.join(".claude").join("plugins");
            fs::create_dir_all(&plugins_dir).unwrap();
            let canonical_mkt = marketplace_dir().unwrap().display().to_string();
            let canonical_plugin = marketplace_dir()
                .unwrap()
                .join("plugins")
                .join("forge-core")
                .display()
                .to_string();

            // Registry already points to canonical (post-migration state)
            fs::write(
                plugins_dir.join("known_marketplaces.json"),
                serde_json::json!({
                    MARKETPLACE_NAME: {
                        "installLocation": canonical_mkt,
                        "source": {"source": "directory", "path": canonical_mkt},
                        "lastUpdated": "2026-05-28T00:00:00Z"
                    }
                })
                .to_string(),
            )
            .unwrap();
            fs::write(
                plugins_dir.join("installed_plugins.json"),
                serde_json::json!({
                    "version": 2,
                    "plugins": {
                        "forge-core@reumbra": [{
                            "scope": "user",
                            "installPath": canonical_plugin,
                            "version": "11.1.0",
                            "installedAt": "2026-05-28T00:00:00Z",
                            "lastUpdated": "2026-05-28T00:00:00Z"
                        }]
                    }
                })
                .to_string(),
            )
            .unwrap();

            // Install another plugin — migration helper runs but should noop on registry
            let second = marketplace_dir().unwrap().join("plugins").join("forge-qa");
            create_plugin_at(&second, "forge-qa", "3.15.3");
            update_marketplace_manifest("forge-qa", "3.15.3", "QA plugin").unwrap();
            integrate_claude_code("forge-qa").unwrap();

            let ip = read_json(plugins_dir.join("installed_plugins.json"));
            // Pre-existing entry still canonical
            assert_eq!(
                ip["plugins"]["forge-core@reumbra"][0]["installPath"],
                canonical_plugin
            );
            // New entry also canonical
            let qa_path = ip["plugins"]["forge-qa@reumbra"][0]["installPath"]
                .as_str()
                .unwrap();
            assert!(
                qa_path.contains(".claude") && qa_path.contains("plugins"),
                "new install path must be under .claude/plugins, got: {qa_path}"
            );
        });
    }

    #[test]
    fn claude_code_enables_plugin_in_settings() {
        with_temp_app_env(|home| {
            create_marketplace_plugin("forge-core", "6.0.0");
            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();

            integrate_claude_code("forge-core").unwrap();

            let settings = read_json(home.join(".claude").join("settings.json"));
            assert_eq!(settings["enabledPlugins"]["forge-core@reumbra"], true);
        });
    }

    #[test]
    fn claude_code_uninstall_removes_settings_entry() {
        with_temp_app_env(|home| {
            create_marketplace_plugin("forge-core", "6.0.0");
            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();
            integrate_claude_code("forge-core").unwrap();

            uninstall_plugin("forge-core", "claude-code").unwrap();

            let settings = read_json(home.join(".claude").join("settings.json"));
            assert!(settings["enabledPlugins"]
                .get("forge-core@reumbra")
                .is_none());
        });
    }

    #[test]
    fn claude_code_uninstall_removes_installed_plugins_entry() {
        with_temp_app_env(|home| {
            create_marketplace_plugin("forge-core", "6.0.0");
            update_marketplace_manifest("forge-core", "6.0.0", "Core plugin").unwrap();
            integrate_claude_code("forge-core").unwrap();

            uninstall_plugin("forge-core", "claude-code").unwrap();

            let installed_plugins = read_json(
                home.join(".claude")
                    .join("plugins")
                    .join("installed_plugins.json"),
            );
            assert!(installed_plugins["plugins"]
                .get("forge-core@reumbra")
                .is_none());
        });
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
        let mut installed_plugins = std::collections::HashMap::new();
        installed_plugins.insert(
            "forge-core".to_string(),
            InstalledPluginEntry {
                version: "6.0.0".to_string(),
                installed_at: "2026-03-08T00:00:00Z".to_string(),
            },
        );
        let config = ForgeConfig {
            license_key: Some("FRG-ABCD-EFGH-IJKL".to_string()),
            plan: Some("pro".to_string()),
            installed_plugins,
            ..Default::default()
        };

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
        let found = ip.get("plugins").and_then(|p| p.get(&plugin_key)).is_some();
        assert!(found, "forge-core should be in cowork");

        let missing_key = format!("{}@{}", "nonexistent", MARKETPLACE_NAME);
        let not_found = ip
            .get("plugins")
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
        let mkt_rel_path = format!(
            "mnt/.claude/cowork_plugins/marketplaces/{}",
            MARKETPLACE_NAME
        );
        let mut km = serde_json::json!({});
        km[MARKETPLACE_NAME] = serde_json::json!({
            "source": { "source": "directory", "path": &mkt_rel_path },
            "installLocation": &mkt_rel_path,
            "lastUpdated": "2026-03-08T00:00:00Z"
        });

        let path = km["reumbra"]["source"]["path"].as_str().unwrap();
        assert!(
            path.starts_with("mnt/"),
            "Cowork paths must start with mnt/"
        );
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

        fs::write(
            &manifest_path,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

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
        assert_eq!(
            manifest.description,
            "Core development pipeline for Claude Code"
        );
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

import { invoke } from '@tauri-apps/api/core';

// Types matching Rust structs

export interface LicenseInfo {
  license_key: string;
  plan: string;
  is_active: boolean;
  expires_at: string;
  machines: MachineInfo[];
  max_machines: number;
}

export interface MachineInfo {
  machine_id: string;
  activated_at: string;
}

export interface PluginInfo {
  name: string;
  description: string | null;
  latest_version: string;
  category: string | null;
}

export interface InstalledPlugin {
  name: string;
  version: string;
  description: string;
  marketplace: string;
  installed_at: string;
  install_path: string;
}

export interface PluginUpdateInfo {
  name: string;
  current_version: string;
  latest_version: string;
  has_update: boolean;
}

export interface AppInfo {
  version: string;
  machine_id: string;
  cowork_detected: boolean;
  cowork_path: string | null;
  os: string;
}

// API functions

export async function activateLicense(licenseKey: string): Promise<LicenseInfo> {
  return invoke('activate_license', { licenseKey });
}

export async function deactivateLicense(): Promise<void> {
  return invoke('deactivate_license');
}

export async function getLicenseStatus(): Promise<LicenseInfo> {
  return invoke('get_license_status');
}

export async function getPluginCatalog(): Promise<PluginInfo[]> {
  return invoke('get_plugin_catalog');
}

export async function installPlugin(pluginName: string, version?: string): Promise<InstalledPlugin> {
  return invoke('install_plugin', { request: { plugin_name: pluginName, version } });
}

export async function uninstallPlugin(pluginName: string): Promise<void> {
  return invoke('uninstall_plugin', { pluginName });
}

export async function getInstalledPlugins(): Promise<InstalledPlugin[]> {
  return invoke('get_installed_plugins');
}

export async function checkPluginUpdates(): Promise<PluginUpdateInfo[]> {
  return invoke('check_plugin_updates');
}

export async function getCoworkPath(): Promise<string | null> {
  return invoke('get_cowork_path');
}

export async function setCoworkPath(path: string): Promise<void> {
  return invoke('set_cowork_path', { path });
}

export async function sendFeedback(feedbackType: string, message: string): Promise<void> {
  return invoke('send_feedback', { feedbackType, message });
}

export async function getAppInfo(): Promise<AppInfo> {
  return invoke('get_app_info');
}

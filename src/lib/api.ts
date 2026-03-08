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

// Environment detection
const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const API_BASE = 'https://api.reumbra.com/velvet';

// State for web mode
let webLicenseKey: string | null = null;
const webMachineId = 'web-browser-' + Math.random().toString(36).slice(2, 10);

// Demo mode for UI testing without real API
let demoMode = false;

export function enableDemoMode() { demoMode = true; }
export function isDemoMode() { return demoMode; }

const DEMO_LICENSE: LicenseInfo = {
  license_key: 'FRG-DEMO-MODE-0001',
  plan: 'bundle',
  is_active: true,
  expires_at: new Date(Date.now() + 365 * 86400000).toISOString(),
  machines: [{ machine_id: webMachineId, activated_at: new Date().toISOString() }],
  max_machines: 3,
};

const DEMO_CATALOG: PluginInfo[] = [
  { name: 'forge-core', description: 'Core orchestrator — setup, upgrade, ecosystem dashboard', latest_version: '4.3.1', category: 'core' },
  { name: 'forge-product', description: 'Product design — user flows, stories, UX criteria', latest_version: '1.2.0', category: 'product' },
  { name: 'forge-tracker', description: 'Task tracking — GitHub Issues, ClickUp, Linear integration', latest_version: '1.1.0', category: 'productivity' },
  { name: 'forge-qa', description: 'QA pipeline — test generation, coverage analysis, test plans', latest_version: '1.0.0', category: 'quality' },
  { name: 'forge-autopilot', description: 'Batch autonomous development with Agent Teams', latest_version: '0.9.0', category: 'automation' },
  { name: 'forge-worktree', description: 'Git worktree management for parallel feature development', latest_version: '0.8.0', category: 'devtools' },
];

async function apiPost<T>(path: string, body: Record<string, unknown>): Promise<T> {
  const resp = await fetch(`${API_BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const data = await resp.json();
  if (!resp.ok) throw new Error(data.error || `API error ${resp.status}`);
  return data as T;
}

async function apiGet<T>(path: string, params: Record<string, string>): Promise<T> {
  const url = new URL(`${API_BASE}${path}`);
  for (const [k, v] of Object.entries(params)) url.searchParams.set(k, v);
  const resp = await fetch(url.toString());
  const data = await resp.json();
  if (!resp.ok) throw new Error(data.error || `API error ${resp.status}`);
  return data as T;
}

// API functions — dual mode (Tauri invoke / Web fetch)

export async function activateLicense(licenseKey: string): Promise<LicenseInfo> {
  if (licenseKey === 'DEMO') {
    demoMode = true;
    return DEMO_LICENSE;
  }
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('activate_license', { licenseKey });
  }
  const info = await apiPost<LicenseInfo>('/auth/activate', {
    license_key: licenseKey,
    machine_id: webMachineId,
  });
  webLicenseKey = licenseKey;
  return info;
}

export async function deactivateLicense(): Promise<void> {
  if (demoMode) { demoMode = false; return; }
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('deactivate_license');
  }
  if (!webLicenseKey) throw new Error('No license activated');
  await apiPost('/auth/deactivate', {
    license_key: webLicenseKey,
    machine_id: webMachineId,
  });
  webLicenseKey = null;
}

export async function getLicenseStatus(): Promise<LicenseInfo> {
  if (demoMode) return DEMO_LICENSE;
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('get_license_status');
  }
  if (!webLicenseKey) throw new Error('No license activated');
  return apiGet<LicenseInfo>('/auth/status', {
    license_key: webLicenseKey,
    machine_id: webMachineId,
  });
}

export async function getPluginCatalog(): Promise<PluginInfo[]> {
  if (demoMode) return DEMO_CATALOG;
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('get_plugin_catalog');
  }
  if (!webLicenseKey) throw new Error('No license activated');
  const data = await apiGet<{ plugins: PluginInfo[] }>('/plugins/list', {
    license_key: webLicenseKey,
    machine_id: webMachineId,
  });
  return data.plugins;
}

export async function installPlugin(pluginName: string, version?: string): Promise<InstalledPlugin> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('install_plugin', { request: { plugin_name: pluginName, version } });
  }
  // Web mode can't install locally — just simulate success
  return {
    name: pluginName,
    version: version || '0.0.0',
    description: '',
    marketplace: 'reumbra-plugins',
    installed_at: new Date().toISOString(),
    install_path: 'web-mode/not-installed',
  };
}

export async function uninstallPlugin(pluginName: string): Promise<void> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('uninstall_plugin', { pluginName });
  }
  // Web mode — no-op
}

export async function getInstalledPlugins(): Promise<InstalledPlugin[]> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('get_installed_plugins');
  }
  // Web mode has no local filesystem
  return [];
}

export async function checkPluginUpdates(): Promise<PluginUpdateInfo[]> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('check_plugin_updates');
  }
  return [];
}

export async function getCoworkPath(): Promise<string | null> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('get_cowork_path');
  }
  return null;
}

export async function setCoworkPath(path: string): Promise<void> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('set_cowork_path', { path });
  }
  // Web mode — no-op
}

export async function sendFeedback(feedbackType: string, message: string): Promise<void> {
  if (demoMode) return;
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('send_feedback', { feedbackType, message });
  }
  await apiPost('/feedback', {
    license_key: webLicenseKey,
    feedback_type: feedbackType,
    message,
    metadata: { source: 'plugin-manager-web', os: navigator.platform },
  });
}

export async function getAppInfo(): Promise<AppInfo> {
  if (isTauri) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke('get_app_info');
  }
  return {
    version: '0.1.0 (web)',
    machine_id: webMachineId,
    cowork_detected: false,
    cowork_path: null,
    os: navigator.platform,
  };
}

import { useState, useEffect } from 'react';
import {
  getInstalledPlugins,
  getAppInfo,
  uninstallPlugin,
  checkPluginUpdates,
  installPlugin,
  type InstalledPlugin,
  type PluginUpdateInfo,
  type AppInfo,
} from '../lib/api';

export default function InstalledPage() {
  const [plugins, setPlugins] = useState<InstalledPlugin[]>([]);
  const [updates, setUpdates] = useState<PluginUpdateInfo[]>([]);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [actionPlugin, setActionPlugin] = useState<string | null>(null);
  const [updatingAll, setUpdatingAll] = useState(false);
  const [error, setError] = useState('');
  const [successMsg, setSuccessMsg] = useState('');

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [inst, upd, info] = await Promise.all([
        getInstalledPlugins(),
        checkPluginUpdates().catch(() => []),
        getAppInfo().catch(() => null),
      ]);
      setPlugins(inst);
      setUpdates(upd);
      setAppInfo(info);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleUninstall = async (name: string) => {
    if (!confirm(`Remove ${name}?`)) return;
    setActionPlugin(name);
    setSuccessMsg('');
    try {
      await uninstallPlugin(name);
      await loadData();
    } catch (err) {
      setError(String(err));
    } finally {
      setActionPlugin(null);
    }
  };

  const handleUpdate = async (name: string) => {
    setActionPlugin(name);
    setError('');
    setSuccessMsg('');
    try {
      await installPlugin(name);
      await loadData();
      showRestartMessage(name);
    } catch (err) {
      setError(String(err));
    } finally {
      setActionPlugin(null);
    }
  };

  const handleUpdateAll = async () => {
    const pluginsWithUpdates = updates.filter((u) => u.has_update);
    if (pluginsWithUpdates.length === 0) return;

    setUpdatingAll(true);
    setError('');
    setSuccessMsg('');

    const updated: string[] = [];
    for (const upd of pluginsWithUpdates) {
      try {
        setActionPlugin(upd.name);
        await installPlugin(upd.name);
        updated.push(upd.name);
      } catch (err) {
        setError(`Failed to update ${upd.name}: ${String(err)}`);
        break;
      }
    }

    setActionPlugin(null);
    setUpdatingAll(false);
    await loadData();

    if (updated.length > 0) {
      showRestartMessage(updated.join(', '));
    }
  };

  const showRestartMessage = (names: string) => {
    if (appInfo?.targets.claude_cowork) {
      setSuccessMsg(`${names} updated. Restart the Claude Cowork app to load changes.`);
    } else {
      setSuccessMsg(`${names} updated. Restart your Claude Code session to load changes.`);
    }
  };

  const getUpdate = (name: string) => updates.find((u) => u.name === name && u.has_update);

  const updatableCount = updates.filter((u) => u.has_update).length;

  // Group plugins by environment
  const claudeCodePlugins = plugins; // All installed plugins go through marketplace
  const hasCowork = appInfo?.targets.claude_cowork;

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="w-6 h-6 border-2 border-forge-500 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-xl font-semibold text-white">Installed Plugins</h2>
          <p className="text-sm text-gray-500 mt-1">
            {plugins.length} plugin{plugins.length !== 1 ? 's' : ''} installed
            {updatableCount > 0 && (
              <span className="text-forge-400 ml-1">
                ({updatableCount} update{updatableCount !== 1 ? 's' : ''} available)
              </span>
            )}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {updatableCount > 0 && (
            <button
              onClick={handleUpdateAll}
              disabled={updatingAll}
              className="px-3 py-1.5 text-xs bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white rounded-lg transition-colors"
            >
              {updatingAll ? 'Updating...' : `Update All (${updatableCount})`}
            </button>
          )}
          <button
            onClick={loadData}
            className="px-3 py-1.5 text-xs text-gray-400 hover:text-white border border-gray-700 hover:border-gray-600 rounded-lg transition-colors"
          >
            Check Updates
          </button>
        </div>
      </div>

      {successMsg && (
        <div className="mb-4 px-3 py-2 bg-green-500/10 border border-green-500/20 rounded-lg flex items-start justify-between">
          <p className="text-green-400 text-xs">{successMsg}</p>
          <button onClick={() => setSuccessMsg('')} className="text-green-600 hover:text-green-400 text-xs ml-2">
            &times;
          </button>
        </div>
      )}

      {error && (
        <div className="mb-4 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
          <p className="text-red-400 text-xs">{error}</p>
        </div>
      )}

      {plugins.length === 0 ? (
        <div className="text-center py-16">
          <p className="text-gray-500 mb-2">No plugins installed yet.</p>
          <p className="text-xs text-gray-600">Go to Catalog to install plugins.</p>
        </div>
      ) : (
        <div className="space-y-6">
          {/* Claude Code section */}
          <EnvironmentSection
            title="Claude Code"
            detected={appInfo?.targets.claude_code ?? false}
            plugins={claudeCodePlugins}
            getUpdate={getUpdate}
            actionPlugin={actionPlugin}
            onUpdate={handleUpdate}
            onUninstall={handleUninstall}
          />

          {/* Claude Cowork section — show if detected */}
          {hasCowork && (
            <EnvironmentSection
              title="Claude Cowork"
              detected
              plugins={claudeCodePlugins}
              getUpdate={getUpdate}
              actionPlugin={actionPlugin}
              onUpdate={handleUpdate}
              onUninstall={handleUninstall}
            />
          )}
        </div>
      )}

      {/* Guides link */}
      <div className="mt-8 pt-6 border-t border-gray-800">
        <p className="text-xs text-gray-600">
          Learn how to use installed plugins:{' '}
          <a
            href="https://forge.reumbra.com/guides"
            target="_blank"
            rel="noopener noreferrer"
            className="text-forge-400 hover:underline"
          >
            Forge Setup Guides
          </a>
        </p>
      </div>
    </div>
  );
}

function EnvironmentSection({
  title,
  detected,
  plugins,
  getUpdate,
  actionPlugin,
  onUpdate,
  onUninstall,
}: {
  title: string;
  detected: boolean;
  plugins: InstalledPlugin[];
  getUpdate: (name: string) => PluginUpdateInfo | undefined;
  actionPlugin: string | null;
  onUpdate: (name: string) => void;
  onUninstall: (name: string) => void;
}) {
  return (
    <div>
      <div className="flex items-center gap-2 mb-3">
        <div className={`w-2 h-2 rounded-full ${detected ? 'bg-green-500' : 'bg-gray-600'}`} />
        <h3 className="text-sm font-medium text-gray-300">{title}</h3>
        {!detected && (
          <span className="text-[10px] text-gray-600">(not detected)</span>
        )}
      </div>

      <div className="space-y-3">
        {plugins.map((plugin) => {
          const update = getUpdate(plugin.name);

          return (
            <div
              key={`${title}-${plugin.name}`}
              className="bg-gray-900 border border-gray-800 rounded-xl p-4 flex items-center gap-4"
            >
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-medium text-white">{plugin.name}</h3>
                  <span className="text-[10px] text-gray-600 font-mono">v{plugin.version}</span>
                  {update && (
                    <span className="px-1.5 py-0.5 bg-forge-600/20 text-forge-300 text-[10px] rounded">
                      v{update.latest_version} available
                    </span>
                  )}
                </div>
                <p className="text-xs text-gray-500 mt-1 truncate">{plugin.description}</p>
              </div>

              <div className="flex items-center gap-2 shrink-0">
                {update && (
                  <button
                    onClick={() => onUpdate(plugin.name)}
                    disabled={actionPlugin === plugin.name}
                    className="px-3 py-1.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 text-white text-xs rounded-lg transition-colors"
                  >
                    {actionPlugin === plugin.name ? '...' : 'Update'}
                  </button>
                )}
                <button
                  onClick={() => onUninstall(plugin.name)}
                  disabled={actionPlugin === plugin.name}
                  className="px-3 py-1.5 text-xs text-red-400 hover:text-red-300 border border-red-500/20 hover:border-red-500/40 rounded-lg transition-colors"
                >
                  Remove
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

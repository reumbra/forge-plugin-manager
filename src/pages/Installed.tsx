import { useState, useEffect } from 'react';
import {
  getInstalledPlugins,
  uninstallPlugin,
  checkPluginUpdates,
  installPlugin,
  type InstalledPlugin,
  type PluginUpdateInfo,
} from '../lib/api';

export default function InstalledPage() {
  const [plugins, setPlugins] = useState<InstalledPlugin[]>([]);
  const [updates, setUpdates] = useState<PluginUpdateInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [actionPlugin, setActionPlugin] = useState<string | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [inst, upd] = await Promise.all([
        getInstalledPlugins(),
        checkPluginUpdates().catch(() => []),
      ]);
      setPlugins(inst);
      setUpdates(upd);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleUninstall = async (name: string) => {
    if (!confirm(`Remove ${name}?`)) return;
    setActionPlugin(name);
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
    try {
      await installPlugin(name);
      await loadData();
    } catch (err) {
      setError(String(err));
    } finally {
      setActionPlugin(null);
    }
  };

  const getUpdate = (name: string) => updates.find((u) => u.name === name && u.has_update);

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
          </p>
        </div>
        <button
          onClick={loadData}
          className="px-3 py-1.5 text-xs text-gray-400 hover:text-white border border-gray-700 hover:border-gray-600 rounded-lg transition-colors"
        >
          Check Updates
        </button>
      </div>

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
        <div className="space-y-3">
          {plugins.map((plugin) => {
            const update = getUpdate(plugin.name);

            return (
              <div
                key={plugin.name}
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
                      onClick={() => handleUpdate(plugin.name)}
                      disabled={actionPlugin === plugin.name}
                      className="px-3 py-1.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 text-white text-xs rounded-lg transition-colors"
                    >
                      {actionPlugin === plugin.name ? '...' : 'Update'}
                    </button>
                  )}
                  <button
                    onClick={() => handleUninstall(plugin.name)}
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
      )}
    </div>
  );
}

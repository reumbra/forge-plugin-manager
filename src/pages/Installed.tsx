import { useState, useEffect } from 'react';
import {
  getInstalledPlugins,
  getAppInfo,
  uninstallPlugin,
  checkPluginUpdates,
  installPlugin,
  type InstalledPlugin,
  type InstallTarget,
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

  const handleUninstall = async (name: string, fromTarget: InstallTarget) => {
    const targetLabel = fromTarget === 'claude-code' ? 'Claude Code' : 'Claude Cowork';
    if (!confirm(`Remove ${name} from ${targetLabel}?`)) return;
    setActionPlugin(name);
    setSuccessMsg('');
    try {
      await uninstallPlugin(name, fromTarget);
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
      setSuccessMsg(`${name} updated. Restart your Claude session to load changes.`);
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
      setSuccessMsg(`${updated.join(', ')} updated. Restart your Claude session to load changes.`);
    }
  };

  const getUpdate = (name: string) => updates.find((u) => u.name === name && u.has_update);
  const updatableCount = updates.filter((u) => u.has_update).length;

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

      {/* Detected environments */}
      {appInfo && (
        <div className="mb-5 flex items-center gap-3">
          <span className="text-xs text-gray-500">Integrated with:</span>
          <div className="flex items-center gap-2">
            {appInfo.targets.claude_cowork && (
              <span className="inline-flex items-center gap-1.5 text-xs text-green-400">
                <span className="w-1.5 h-1.5 bg-green-500 rounded-full" />
                Claude Cowork
              </span>
            )}
            {appInfo.targets.claude_code && (
              <span className="inline-flex items-center gap-1.5 text-xs text-green-400">
                <span className="w-1.5 h-1.5 bg-green-500 rounded-full" />
                Claude Code
              </span>
            )}
            {!appInfo.targets.claude_code && !appInfo.targets.claude_cowork && (
              <span className="text-xs text-yellow-400">No Claude environment detected</span>
            )}
          </div>
        </div>
      )}

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
        <div className="space-y-3">
          {plugins.map((plugin) => {
            const update = getUpdate(plugin.name);
            const targets = plugin.targets || [];
            const inCode = targets.includes('claude-code');
            const inCowork = targets.includes('claude-cowork');

            return (
              <div
                key={plugin.name}
                className="bg-gray-900 border border-gray-800 rounded-xl p-4"
              >
                <div className="flex items-center gap-4">
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

                  {update && (
                    <button
                      onClick={() => handleUpdate(plugin.name)}
                      disabled={actionPlugin === plugin.name}
                      className="px-3 py-1.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 text-white text-xs rounded-lg transition-colors shrink-0"
                    >
                      {actionPlugin === plugin.name ? '...' : 'Update'}
                    </button>
                  )}
                </div>

                {/* Target badges + remove buttons */}
                <div className="mt-3 flex items-center gap-2 flex-wrap">
                  {inCowork && (
                    <div className="inline-flex items-center gap-1.5 px-2 py-1 bg-purple-500/10 border border-purple-500/20 rounded text-[10px]">
                      <span className="w-1.5 h-1.5 bg-purple-400 rounded-full" />
                      <span className="text-purple-300">Cowork</span>
                      <button
                        onClick={() => handleUninstall(plugin.name, 'claude-cowork')}
                        disabled={actionPlugin === plugin.name}
                        className="text-purple-600 hover:text-red-400 ml-1"
                        title="Remove from Claude Cowork"
                      >
                        &times;
                      </button>
                    </div>
                  )}
                  {inCode && (
                    <div className="inline-flex items-center gap-1.5 px-2 py-1 bg-blue-500/10 border border-blue-500/20 rounded text-[10px]">
                      <span className="w-1.5 h-1.5 bg-blue-400 rounded-full" />
                      <span className="text-blue-300">Code</span>
                      <button
                        onClick={() => handleUninstall(plugin.name, 'claude-code')}
                        disabled={actionPlugin === plugin.name}
                        className="text-blue-600 hover:text-red-400 ml-1"
                        title="Remove from Claude Code"
                      >
                        &times;
                      </button>
                    </div>
                  )}
                  {!inCode && !inCowork && (
                    <span className="text-[10px] text-gray-600">Not integrated (files only)</span>
                  )}
                </div>
              </div>
            );
          })}
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

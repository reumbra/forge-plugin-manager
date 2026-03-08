import { useState, useEffect } from 'react';
import {
  getPluginCatalog,
  getInstalledPlugins,
  installPlugin,
  type PluginInfo,
  type InstalledPlugin,
  type LicenseInfo,
} from '../lib/api';

interface Props {
  license: LicenseInfo;
}

export default function CatalogPage({ license }: Props) {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [installed, setInstalled] = useState<InstalledPlugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [catalog, inst] = await Promise.all([
        getPluginCatalog(),
        getInstalledPlugins().catch(() => []),
      ]);
      setPlugins(catalog);
      setInstalled(inst);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleInstall = async (pluginName: string) => {
    setInstalling(pluginName);
    setError('');
    try {
      await installPlugin(pluginName);
      // Refresh installed list
      const inst = await getInstalledPlugins().catch(() => []);
      setInstalled(inst);
    } catch (err) {
      setError(String(err));
    } finally {
      setInstalling(null);
    }
  };

  const isInstalled = (name: string) => installed.some((p) => p.name === name);
  const getInstalledVersion = (name: string) => installed.find((p) => p.name === name)?.version;

  if (loading) {
    return <LoadingState />;
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h2 className="text-xl font-semibold text-white">Plugin Catalog</h2>
          <p className="text-sm text-gray-500 mt-1">
            Available plugins for your {license.plan} plan
          </p>
        </div>
        <button
          onClick={loadData}
          className="px-3 py-1.5 text-xs text-gray-400 hover:text-white border border-gray-700 hover:border-gray-600 rounded-lg transition-colors"
        >
          Refresh
        </button>
      </div>

      {error && (
        <div className="mb-4 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
          <p className="text-red-400 text-xs">{error}</p>
        </div>
      )}

      {plugins.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-gray-500">No plugins available for your plan.</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {plugins.map((plugin) => {
            const installed = isInstalled(plugin.name);
            const installedVersion = getInstalledVersion(plugin.name);
            const hasUpdate = installed && installedVersion !== plugin.latest_version;

            return (
              <div
                key={plugin.name}
                className="bg-gray-900 border border-gray-800 rounded-xl p-5 hover:border-gray-700 transition-colors"
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1 min-w-0">
                    <h3 className="text-sm font-medium text-white">{plugin.name}</h3>
                    {plugin.category && (
                      <span className="inline-block mt-1 px-2 py-0.5 bg-gray-800 text-gray-400 text-[10px] rounded-full">
                        {plugin.category}
                      </span>
                    )}
                  </div>
                  <span className="text-[10px] text-gray-600 font-mono ml-2">
                    v{plugin.latest_version}
                  </span>
                </div>

                <p className="text-xs text-gray-500 mt-2 line-clamp-2">
                  {plugin.description || 'No description available'}
                </p>

                <div className="mt-4">
                  {installed && !hasUpdate ? (
                    <span className="inline-flex items-center gap-1.5 text-xs text-green-400">
                      <span className="w-1.5 h-1.5 bg-green-400 rounded-full" />
                      Installed (v{installedVersion})
                    </span>
                  ) : (
                    <button
                      onClick={() => handleInstall(plugin.name)}
                      disabled={installing === plugin.name}
                      className="px-4 py-1.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white text-xs rounded-lg transition-colors"
                    >
                      {installing === plugin.name
                        ? 'Installing...'
                        : hasUpdate
                          ? `Update to v${plugin.latest_version}`
                          : 'Install'}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function LoadingState() {
  return (
    <div className="space-y-4">
      {[1, 2, 3].map((i) => (
        <div key={i} className="bg-gray-900 border border-gray-800 rounded-xl p-5 animate-pulse">
          <div className="h-4 bg-gray-800 rounded w-1/3 mb-3" />
          <div className="h-3 bg-gray-800 rounded w-2/3" />
        </div>
      ))}
    </div>
  );
}

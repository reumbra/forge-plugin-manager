import { useState, useEffect } from 'react';
import {
  getPluginCatalog,
  getInstalledPlugins,
  getAppInfo,
  installPlugin,
  type PluginInfo,
  type InstalledPlugin,
  type LicenseInfo,
  type AppInfo,
} from '../lib/api';

interface Props {
  license: LicenseInfo;
}

type InstallTarget = 'claude-code' | 'claude-cowork';

// Dependency graph — defines catalog structure
const CATALOG_SECTIONS = [
  {
    id: 'foundations',
    title: 'Foundations',
    description: 'Core modules required by all plugins',
    plugins: ['forge-core', 'forge-product'],
  },
  {
    id: 'devtools',
    title: 'Development Tools',
    description: 'QA, tracking, automation, and parallel development',
    plugins: ['forge-qa', 'forge-tracker', 'forge-autopilot', 'forge-worktree'],
  },
  {
    id: 'advisors',
    title: 'Product Advisors',
    description: 'Strategy and growth modules — powered by forge-product',
    requires: 'forge-product',
    plugins: [
      'forge-discovery',
      'forge-marketing',
      'forge-analytics',
      'forge-onboarding',
      'forge-copy',
      'forge-seo',
      'forge-growth',
      'forge-ab',
    ],
  },
] as const;

// Short labels for advisor plugins (drop "forge-" prefix)
const SHORT_LABELS: Record<string, string> = {
  'forge-discovery': 'Discovery',
  'forge-marketing': 'Marketing',
  'forge-analytics': 'Analytics',
  'forge-onboarding': 'Onboarding',
  'forge-copy': 'Copywriting',
  'forge-seo': 'SEO',
  'forge-growth': 'Growth',
  'forge-ab': 'A/B Testing',
  'forge-prompts': 'Prompts',
};

export default function CatalogPage({ license }: Props) {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [installed, setInstalled] = useState<InstalledPlugin[]>([]);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [successMsg, setSuccessMsg] = useState('');
  const [target, setTarget] = useState<InstallTarget>('claude-code');

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [catalog, inst, info] = await Promise.all([
        getPluginCatalog(),
        getInstalledPlugins().catch(() => []),
        getAppInfo().catch(() => null),
      ]);
      setPlugins(catalog);
      setInstalled(inst);
      setAppInfo(info);

      if (info) {
        if (info.targets.claude_code && !info.targets.claude_cowork) setTarget('claude-code');
        else if (!info.targets.claude_code && info.targets.claude_cowork) setTarget('claude-cowork');
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleInstall = async (pluginName: string) => {
    setInstalling(pluginName);
    setError('');
    setSuccessMsg('');
    try {
      await installPlugin(pluginName);
      const inst = await getInstalledPlugins().catch(() => []);
      setInstalled(inst);

      if (target === 'claude-code') {
        setSuccessMsg(`${pluginName} installed. Restart your Claude Code session to load it.`);
      } else {
        setSuccessMsg(`${pluginName} installed. Restart the Claude Cowork app to load it.`);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setInstalling(null);
    }
  };

  const getPlugin = (name: string) => plugins.find((p) => p.name === name);
  const isInstalled = (name: string) => installed.some((p) => p.name === name);
  const getInstalledVersion = (name: string) => installed.find((p) => p.name === name)?.version;

  const hasTargets = appInfo && (appInfo.targets.claude_code || appInfo.targets.claude_cowork);
  const hasBothTargets = appInfo?.targets.claude_code && appInfo?.targets.claude_cowork;

  if (loading) return <LoadingState />;

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

      {/* Target selector */}
      {hasTargets && (
        <div className="mb-5 flex items-center gap-3">
          <span className="text-xs text-gray-500">Install to:</span>
          {hasBothTargets ? (
            <div className="flex rounded-lg border border-gray-700 overflow-hidden">
              <button
                onClick={() => setTarget('claude-code')}
                className={`px-3 py-1.5 text-xs transition-colors ${
                  target === 'claude-code' ? 'bg-forge-600/20 text-forge-300' : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                Claude Code
              </button>
              <button
                onClick={() => setTarget('claude-cowork')}
                className={`px-3 py-1.5 text-xs border-l border-gray-700 transition-colors ${
                  target === 'claude-cowork' ? 'bg-forge-600/20 text-forge-300' : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                Claude Cowork
              </button>
            </div>
          ) : (
            <span className="text-xs text-gray-300">
              {appInfo?.targets.claude_code ? 'Claude Code' : 'Claude Cowork'}
            </span>
          )}
        </div>
      )}

      {!hasTargets && (
        <div className="mb-5 px-3 py-2 bg-yellow-500/10 border border-yellow-500/20 rounded-lg">
          <p className="text-yellow-400 text-xs">
            Neither Claude Code nor Claude Cowork detected. Plugins will be installed to the default marketplace directory.
          </p>
        </div>
      )}

      {successMsg && (
        <div className="mb-4 px-3 py-2 bg-green-500/10 border border-green-500/20 rounded-lg flex items-start justify-between">
          <p className="text-green-400 text-xs">{successMsg}</p>
          <button onClick={() => setSuccessMsg('')} className="text-green-600 hover:text-green-400 text-xs ml-2">&times;</button>
        </div>
      )}

      {error && (
        <div className="mb-4 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
          <p className="text-red-400 text-xs">{error}</p>
        </div>
      )}

      {/* Catalog sections */}
      <div className="space-y-8">
        {CATALOG_SECTIONS.map((section) => {
          const sectionPlugins = section.plugins
            .map((name) => getPlugin(name))
            .filter((p): p is PluginInfo => p !== undefined);

          if (sectionPlugins.length === 0) return null;

          const requiresMet = !('requires' in section) || isInstalled(section.requires!);

          return (
            <div key={section.id}>
              <div className="mb-3">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-medium text-gray-300">{section.title}</h3>
                  {'requires' in section && (
                    <span className={`text-[10px] px-1.5 py-0.5 rounded ${
                      requiresMet
                        ? 'bg-green-500/10 text-green-400'
                        : 'bg-yellow-500/10 text-yellow-400'
                    }`}>
                      {requiresMet ? 'forge-product installed' : 'requires forge-product'}
                    </span>
                  )}
                </div>
                <p className="text-xs text-gray-600 mt-0.5">{section.description}</p>
              </div>

              {section.id === 'advisors' ? (
                // Compact grid for advisors
                <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
                  {sectionPlugins.map((plugin) => {
                    const pluginInstalled = isInstalled(plugin.name);
                    const installedVersion = getInstalledVersion(plugin.name);
                    const hasUpdate = pluginInstalled && installedVersion !== plugin.latest_version;

                    return (
                      <div
                        key={plugin.name}
                        className="bg-gray-900 border border-gray-800 rounded-lg p-3 hover:border-gray-700 transition-colors"
                      >
                        <div className="flex items-center justify-between mb-1">
                          <h4 className="text-xs font-medium text-white">
                            {SHORT_LABELS[plugin.name] || plugin.name}
                          </h4>
                          <span className="text-[9px] text-gray-600 font-mono">v{plugin.latest_version}</span>
                        </div>
                        <p className="text-[10px] text-gray-500 line-clamp-2 mb-2 min-h-[2rem]">
                          {plugin.description}
                        </p>
                        {pluginInstalled && !hasUpdate ? (
                          <span className="inline-flex items-center gap-1 text-[10px] text-green-400">
                            <span className="w-1 h-1 bg-green-400 rounded-full" />
                            v{installedVersion}
                          </span>
                        ) : (
                          <button
                            onClick={() => handleInstall(plugin.name)}
                            disabled={installing === plugin.name}
                            className="px-2.5 py-1 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white text-[10px] rounded transition-colors"
                          >
                            {installing === plugin.name
                              ? '...'
                              : hasUpdate
                                ? `Update`
                                : 'Install'}
                          </button>
                        )}
                      </div>
                    );
                  })}
                </div>
              ) : (
                // Full cards for foundations and devtools
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {sectionPlugins.map((plugin) => {
                    const pluginInstalled = isInstalled(plugin.name);
                    const installedVersion = getInstalledVersion(plugin.name);
                    const hasUpdate = pluginInstalled && installedVersion !== plugin.latest_version;
                    const isCore = plugin.name === 'forge-core';

                    return (
                      <div
                        key={plugin.name}
                        className={`bg-gray-900 border rounded-xl p-5 hover:border-gray-700 transition-colors ${
                          isCore ? 'border-forge-600/30' : 'border-gray-800'
                        }`}
                      >
                        <div className="flex items-start justify-between">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <h4 className="text-sm font-medium text-white">{plugin.name}</h4>
                              {isCore && (
                                <span className="text-[9px] px-1.5 py-0.5 bg-forge-600/20 text-forge-300 rounded">
                                  required
                                </span>
                              )}
                              {plugin.name === 'forge-product' && (
                                <span className="text-[9px] px-1.5 py-0.5 bg-purple-600/20 text-purple-300 rounded">
                                  hub
                                </span>
                              )}
                            </div>
                          </div>
                          <span className="text-[10px] text-gray-600 font-mono ml-2">
                            v{plugin.latest_version}
                          </span>
                        </div>

                        <p className="text-xs text-gray-500 mt-2 line-clamp-2">
                          {plugin.description || 'No description available'}
                        </p>

                        <div className="mt-4">
                          {pluginInstalled && !hasUpdate ? (
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
        })}
      </div>

      {/* Guides link */}
      <div className="mt-8 pt-6 border-t border-gray-800">
        <p className="text-xs text-gray-600">
          New to Forge?{' '}
          <a
            href="https://forge.reumbra.com/guides"
            target="_blank"
            rel="noopener noreferrer"
            className="text-forge-400 hover:underline"
          >
            Read the setup guides
          </a>
          {' '}to learn how to use plugins with Claude Code and Cowork.
        </p>
      </div>
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

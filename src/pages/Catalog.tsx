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
  type InstallTarget,
} from '../lib/api';

interface Props {
  license: LicenseInfo;
}

// Category display config — order + labels + descriptions
const CATEGORY_CONFIG: Record<string, { title: string; description: string; layout: 'cards' | 'grid' }> = {
  foundations: {
    title: 'Foundations',
    description: 'Core modules required by all plugins',
    layout: 'cards',
  },
  devtools: {
    title: 'Development Tools',
    description: 'QA, tracking, automation, and parallel development',
    layout: 'cards',
  },
  advisors: {
    title: 'Product Advisors',
    description: 'Strategy and growth modules — powered by forge-product',
    layout: 'grid',
  },
};

// Display order for known categories
const CATEGORY_ORDER = ['foundations', 'devtools', 'advisors'];

// Short labels for compact grid layout
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
  'forge-init': 'Init',
};

export default function CatalogPage({ license }: Props) {
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [installed, setInstalled] = useState<InstalledPlugin[]>([]);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState<string | null>(null);
  const [error, setError] = useState('');
  const [successMsg, setSuccessMsg] = useState('');
  const [target, setTarget] = useState<InstallTarget>('claude-cowork');

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

      // Auto-select available target (prefer Cowork)
      if (info) {
        if (info.targets.claude_cowork && !info.targets.claude_code) setTarget('claude-cowork');
        else if (!info.targets.claude_cowork && info.targets.claude_code) setTarget('claude-code');
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
      await installPlugin(pluginName, target);
      const inst = await getInstalledPlugins().catch(() => []);
      setInstalled(inst);

      const targetLabel = target === 'claude-cowork' ? 'Claude Cowork' : 'Claude Code';
      setSuccessMsg(`${pluginName} installed to ${targetLabel}. Restart your session to load it.`);
    } catch (err) {
      setError(String(err));
    } finally {
      setInstalling(null);
    }
  };

  const isInstalledForTarget = (name: string) => {
    const plugin = installed.find((p) => p.name === name);
    return plugin?.targets?.includes(target) ?? false;
  };
  const getInstalledVersion = (name: string) => installed.find((p) => p.name === name)?.version;

  const hasTargets = appInfo && (appInfo.targets.claude_code || appInfo.targets.claude_cowork);
  const hasBothTargets = appInfo?.targets.claude_code && appInfo?.targets.claude_cowork;

  // Group plugins by category dynamically
  const groupedPlugins = () => {
    const groups: { key: string; title: string; description: string; layout: 'cards' | 'grid'; plugins: PluginInfo[] }[] = [];
    const usedCategories = new Set<string>();

    // First: known categories in defined order
    for (const catKey of CATEGORY_ORDER) {
      const catPlugins = plugins.filter((p) => p.category === catKey);
      if (catPlugins.length === 0) continue;
      const config = CATEGORY_CONFIG[catKey];
      groups.push({
        key: catKey,
        title: config.title,
        description: config.description,
        layout: config.layout,
        plugins: catPlugins,
      });
      usedCategories.add(catKey);
    }

    // Then: unknown categories (new ones from API) + null category → "Other"
    const otherPlugins = plugins.filter(
      (p) => !p.category || !usedCategories.has(p.category)
    );
    if (otherPlugins.length > 0) {
      groups.push({
        key: 'other',
        title: 'Other',
        description: 'Additional plugins',
        layout: 'grid',
        plugins: otherPlugins,
      });
    }

    return groups;
  };

  if (loading) return <LoadingState />;

  const sections = groupedPlugins();

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

      {/* Target selector — Cowork first */}
      {hasTargets && (
        <div className="mb-5 flex items-center gap-3">
          <span className="text-xs text-gray-500">Install to:</span>
          {hasBothTargets ? (
            <div className="flex rounded-lg border border-gray-700 overflow-hidden">
              <button
                onClick={() => setTarget('claude-cowork')}
                className={`px-3 py-1.5 text-xs transition-colors ${
                  target === 'claude-cowork' ? 'bg-forge-600/20 text-forge-300' : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                Claude Cowork
              </button>
              <button
                onClick={() => setTarget('claude-code')}
                className={`px-3 py-1.5 text-xs border-l border-gray-700 transition-colors ${
                  target === 'claude-code' ? 'bg-forge-600/20 text-forge-300' : 'text-gray-400 hover:text-gray-200'
                }`}
              >
                Claude Code
              </button>
            </div>
          ) : (
            <span className="text-xs text-gray-300">
              {appInfo?.targets.claude_cowork ? 'Claude Cowork' : 'Claude Code'}
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

      {/* Dynamic catalog sections */}
      <div className="space-y-8">
        {sections.map((section) => {
          const requiresProduct = section.key === 'advisors';
          const requiresMet = !requiresProduct || isInstalledForTarget('forge-product');

          return (
            <div key={section.key}>
              <div className="mb-3">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-medium text-gray-300">{section.title}</h3>
                  {requiresProduct && (
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

              {section.layout === 'grid' ? (
                <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
                  {section.plugins.map((plugin) => (
                    <CompactCard
                      key={plugin.name}
                      plugin={plugin}
                      installed={isInstalledForTarget(plugin.name)}
                      installedVersion={getInstalledVersion(plugin.name)}
                      installing={installing === plugin.name}
                      onInstall={() => handleInstall(plugin.name)}
                    />
                  ))}
                </div>
              ) : (
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  {section.plugins.map((plugin) => (
                    <FullCard
                      key={plugin.name}
                      plugin={plugin}
                      installed={isInstalledForTarget(plugin.name)}
                      installedVersion={getInstalledVersion(plugin.name)}
                      installing={installing === plugin.name}
                      onInstall={() => handleInstall(plugin.name)}
                    />
                  ))}
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

function CompactCard({ plugin, installed, installedVersion, installing, onInstall }: {
  plugin: PluginInfo;
  installed: boolean;
  installedVersion: string | undefined;
  installing: boolean;
  onInstall: () => void;
}) {
  const hasUpdate = installed && installedVersion !== plugin.latest_version;

  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-3 hover:border-gray-700 transition-colors">
      <div className="flex items-center justify-between mb-1">
        <h4 className="text-xs font-medium text-white">
          {SHORT_LABELS[plugin.name] || plugin.name.replace('forge-', '')}
        </h4>
        <span className="text-[9px] text-gray-600 font-mono">v{plugin.latest_version}</span>
      </div>
      <p className="text-[10px] text-gray-500 line-clamp-2 mb-2 min-h-[2rem]">
        {plugin.description}
      </p>
      {installed && !hasUpdate ? (
        <span className="inline-flex items-center gap-1 text-[10px] text-green-400">
          <span className="w-1 h-1 bg-green-400 rounded-full" />
          v{installedVersion}
        </span>
      ) : (
        <button
          onClick={onInstall}
          disabled={installing}
          className="px-2.5 py-1 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white text-[10px] rounded transition-colors"
        >
          {installing ? '...' : hasUpdate ? 'Update' : 'Install'}
        </button>
      )}
    </div>
  );
}

function FullCard({ plugin, installed, installedVersion, installing, onInstall }: {
  plugin: PluginInfo;
  installed: boolean;
  installedVersion: string | undefined;
  installing: boolean;
  onInstall: () => void;
}) {
  const hasUpdate = installed && installedVersion !== plugin.latest_version;
  const isCore = plugin.name === 'forge-core';
  const isHub = plugin.name === 'forge-product';

  return (
    <div className={`bg-gray-900 border rounded-xl p-5 hover:border-gray-700 transition-colors ${
      isCore ? 'border-forge-600/30' : 'border-gray-800'
    }`}>
      <div className="flex items-start justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h4 className="text-sm font-medium text-white">{plugin.name}</h4>
            {isCore && (
              <span className="text-[9px] px-1.5 py-0.5 bg-forge-600/20 text-forge-300 rounded">
                required
              </span>
            )}
            {isHub && (
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
        {installed && !hasUpdate ? (
          <span className="inline-flex items-center gap-1.5 text-xs text-green-400">
            <span className="w-1.5 h-1.5 bg-green-400 rounded-full" />
            Installed (v{installedVersion})
          </span>
        ) : (
          <button
            onClick={onInstall}
            disabled={installing}
            className="px-4 py-1.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white text-xs rounded-lg transition-colors"
          >
            {installing
              ? 'Installing...'
              : hasUpdate
                ? `Update to v${plugin.latest_version}`
                : 'Install'}
          </button>
        )}
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

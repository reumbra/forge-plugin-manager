import { useState, useEffect } from 'react';
import {
  deactivateLicense,
  getAppInfo,
  type AppInfo,
  type LicenseInfo,
} from '../lib/api';

interface Props {
  license: LicenseInfo;
  onDeactivated: () => void;
}

export default function SettingsPage({ license, onDeactivated }: Props) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [deactivating, setDeactivating] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    getAppInfo().then(setAppInfo);
  }, []);

  const handleDeactivate = async () => {
    if (!confirm('Deactivate this license? You can reactivate it later.')) return;
    setDeactivating(true);
    try {
      await deactivateLicense();
      onDeactivated();
    } catch (err) {
      setError(String(err));
      setDeactivating(false);
    }
  };

  const expiresAt = new Date(license.expires_at);
  const daysLeft = Math.ceil((expiresAt.getTime() - Date.now()) / (1000 * 60 * 60 * 24));

  return (
    <div className="max-w-lg">
      <h2 className="text-xl font-semibold text-white mb-6">Settings</h2>

      {error && (
        <div className="mb-4 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
          <p className="text-red-400 text-xs">{error}</p>
        </div>
      )}

      {/* License */}
      <Section title="License">
        <InfoRow label="Key" value={license.license_key} mono />
        <InfoRow label="Plan" value={license.plan.charAt(0).toUpperCase() + license.plan.slice(1)} />
        <InfoRow
          label="Expires"
          value={`${expiresAt.toLocaleDateString()} (${daysLeft}d left)`}
          warn={daysLeft < 30}
        />
        <InfoRow
          label="Devices"
          value={`${license.machines.length} / ${license.max_machines}`}
        />
        <div className="mt-3">
          <button
            onClick={handleDeactivate}
            disabled={deactivating}
            className="px-3 py-1.5 text-xs text-red-400 hover:text-red-300 border border-red-500/20 hover:border-red-500/40 rounded-lg transition-colors"
          >
            {deactivating ? 'Deactivating...' : 'Deactivate License'}
          </button>
        </div>
      </Section>

      {/* Environment */}
      {appInfo && (
        <Section title="Environment">
          <InfoRow
            label="Cowork Spaces"
            value={appInfo.targets.cowork_spaces.length > 0 ? `${appInfo.targets.cowork_spaces.length} detected` : 'Not found'}
            warn={appInfo.targets.cowork_spaces.length === 0}
          />
          {appInfo.targets.cowork_spaces.map((s) => (
            <InfoRow key={s.id} label="" value={`${s.label}${s.is_org ? ' (org)' : ''} — ${s.path}`} mono small />
          ))}
          <InfoRow
            label="Claude Code"
            value={appInfo.targets.claude_code ? 'Detected' : 'Not found'}
            warn={!appInfo.targets.claude_code}
          />
          {appInfo.targets.claude_code_path && (
            <InfoRow label="" value={appInfo.targets.claude_code_path} mono small />
          )}
          {appInfo.config_dir && (
            <InfoRow label="Plugin storage" value={appInfo.config_dir} mono small />
          )}
        </Section>
      )}

      {/* About */}
      {appInfo && (
        <Section title="About">
          <InfoRow label="Version" value={`v${appInfo.version}`} />
          <InfoRow label="OS" value={appInfo.os} />
          <InfoRow label="Machine ID" value={appInfo.machine_id.slice(0, 16) + '...'} mono />
        </Section>
      )}

      {/* Links */}
      <Section title="Resources">
        <div className="space-y-2">
          <ResourceLink
            href="https://forge.reumbra.com/guides"
            label="Setup Guides"
            description="How to use Forge plugins with Claude Code and Cowork"
          />
          <ResourceLink
            href="https://forge.reumbra.com/pricing"
            label="Plans & Pricing"
            description="Upgrade your plan or manage subscription"
          />
        </div>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-6 pb-6 border-b border-gray-800 last:border-0">
      <h3 className="text-sm font-medium text-gray-300 mb-3">{title}</h3>
      {children}
    </div>
  );
}

function InfoRow({
  label,
  value,
  mono,
  warn,
  small,
}: {
  label: string;
  value: string;
  mono?: boolean;
  warn?: boolean;
  small?: boolean;
}) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-xs text-gray-500">{label}</span>
      <span
        className={`${small ? 'text-[10px]' : 'text-xs'} ${warn ? 'text-yellow-400' : 'text-gray-300'} ${mono ? 'font-mono' : ''}`}
      >
        {value}
      </span>
    </div>
  );
}

function ResourceLink({ href, label, description }: { href: string; label: string; description: string }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="flex items-center justify-between py-2 px-3 -mx-3 rounded-lg hover:bg-gray-800/50 transition-colors group"
    >
      <div>
        <span className="text-xs text-forge-400 group-hover:underline">{label}</span>
        <p className="text-[10px] text-gray-600">{description}</p>
      </div>
      <svg className="w-3.5 h-3.5 text-gray-600 group-hover:text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
        <path strokeLinecap="round" strokeLinejoin="round" d="M13.5 6H5.25A2.25 2.25 0 003 8.25v10.5A2.25 2.25 0 005.25 21h10.5A2.25 2.25 0 0018 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25" />
      </svg>
    </a>
  );
}

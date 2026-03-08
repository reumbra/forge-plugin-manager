import { useState, useEffect } from 'react';
import {
  deactivateLicense,
  getAppInfo,
  getCoworkPath,
  setCoworkPath,
  type AppInfo,
  type LicenseInfo,
} from '../lib/api';

interface Props {
  license: LicenseInfo;
  onDeactivated: () => void;
}

export default function SettingsPage({ license, onDeactivated }: Props) {
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [coworkPath, setCoworkPathState] = useState<string>('');
  const [pathError, setPathError] = useState('');
  const [deactivating, setDeactivating] = useState(false);

  useEffect(() => {
    Promise.all([getAppInfo(), getCoworkPath()]).then(([info, path]) => {
      setAppInfo(info);
      setCoworkPathState(path || '');
    });
  }, []);

  const handleSavePath = async () => {
    setPathError('');
    try {
      await setCoworkPath(coworkPath);
    } catch (err) {
      setPathError(String(err));
    }
  };

  const handleDeactivate = async () => {
    if (!confirm('Deactivate this license? You can reactivate it later.')) return;
    setDeactivating(true);
    try {
      await deactivateLicense();
      onDeactivated();
    } catch (err) {
      setPathError(String(err));
      setDeactivating(false);
    }
  };

  const expiresAt = new Date(license.expires_at);
  const daysLeft = Math.ceil((expiresAt.getTime() - Date.now()) / (1000 * 60 * 60 * 24));

  return (
    <div className="max-w-lg">
      <h2 className="text-xl font-semibold text-white mb-6">Settings</h2>

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

      {/* Cowork Path */}
      <Section title="Cowork Directory">
        <p className="text-xs text-gray-500 mb-3">
          Path to Claude Cowork plugins directory. Auto-detected on startup.
        </p>
        <div className="flex gap-2">
          <input
            type="text"
            value={coworkPath}
            onChange={(e) => setCoworkPathState(e.target.value)}
            placeholder="Auto-detect failed — enter path manually"
            className="flex-1 px-3 py-2 bg-gray-800 border border-gray-700 rounded-lg text-white text-xs font-mono focus:outline-none focus:border-forge-500 transition-colors"
          />
          <button
            onClick={handleSavePath}
            className="px-3 py-2 bg-gray-800 hover:bg-gray-700 text-gray-300 text-xs rounded-lg border border-gray-700 transition-colors"
          >
            Save
          </button>
        </div>
        {pathError && <p className="text-red-400 text-xs mt-2">{pathError}</p>}
      </Section>

      {/* App Info */}
      {appInfo && (
        <Section title="About">
          <InfoRow label="Version" value={`v${appInfo.version}`} />
          <InfoRow label="OS" value={appInfo.os} />
          <InfoRow label="Machine ID" value={appInfo.machine_id.slice(0, 16) + '...'} mono />
          <InfoRow
            label="Claude Code"
            value={appInfo.targets.claude_code ? 'Detected' : 'Not found'}
            warn={!appInfo.targets.claude_code}
          />
          <InfoRow
            label="Claude Cowork"
            value={appInfo.targets.claude_cowork ? 'Detected' : 'Not found'}
            warn={!appInfo.targets.claude_cowork}
          />
        </Section>
      )}
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
}: {
  label: string;
  value: string;
  mono?: boolean;
  warn?: boolean;
}) {
  return (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-xs text-gray-500">{label}</span>
      <span
        className={`text-xs ${warn ? 'text-yellow-400' : 'text-gray-300'} ${mono ? 'font-mono' : ''}`}
      >
        {value}
      </span>
    </div>
  );
}

import { useState } from 'react';
import { activateLicense, type LicenseInfo } from '../lib/api';

interface Props {
  onActivated: (license: LicenseInfo) => void;
}

export default function ActivationPage({ onActivated }: Props) {
  const [key, setKey] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleActivate = async () => {
    const trimmed = key.trim();
    if (!trimmed) return;

    setLoading(true);
    setError('');

    try {
      const license = await activateLicense(trimmed);
      onActivated(license);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') handleActivate();
  };

  return (
    <div className="flex items-center justify-center h-screen bg-gray-950">
      <div className="w-full max-w-sm px-6">
        {/* Logo */}
        <div className="text-center mb-8">
          <h1 className="text-2xl font-bold text-white">Forge</h1>
          <p className="text-sm text-gray-500 mt-1">Plugin Manager</p>
        </div>

        {/* Activation form */}
        <div className="space-y-4">
          <div>
            <label htmlFor="license-key" className="block text-sm text-gray-400 mb-2">
              License Key
            </label>
            <input
              id="license-key"
              type="text"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="FRG-XXXX-XXXX-XXXX"
              className="w-full px-4 py-2.5 bg-gray-900 border border-gray-700 rounded-lg text-white placeholder-gray-600 font-mono text-sm focus:outline-none focus:border-forge-500 focus:ring-1 focus:ring-forge-500 transition-colors"
              disabled={loading}
              autoFocus
            />
          </div>

          {error && (
            <div className="px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg">
              <p className="text-red-400 text-xs">{error}</p>
            </div>
          )}

          <button
            onClick={handleActivate}
            disabled={loading || !key.trim()}
            className="w-full py-2.5 bg-forge-600 hover:bg-forge-700 disabled:bg-gray-800 disabled:text-gray-600 text-white rounded-lg text-sm font-medium transition-colors"
          >
            {loading ? 'Activating...' : 'Activate'}
          </button>
        </div>

        <p className="text-center text-[11px] text-gray-600 mt-6">
          Purchase a license at{' '}
          <a href="https://forge.reumbra.com/pricing" className="text-forge-400 hover:underline">
            forge.reumbra.com/pricing
          </a>
        </p>
      </div>
    </div>
  );
}

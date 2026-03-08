import { useState, useEffect } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import Sidebar from './components/Sidebar';
import ActivationPage from './pages/Activation';
import CatalogPage from './pages/Catalog';
import InstalledPage from './pages/Installed';
import SettingsPage from './pages/Settings';
import FeedbackPage from './pages/Feedback';
import { getLicenseStatus, type LicenseInfo } from './lib/api';

export type Page = 'catalog' | 'installed' | 'settings' | 'feedback';

function App() {
  const [license, setLicense] = useState<LicenseInfo | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getLicenseStatus()
      .then(setLicense)
      .catch(() => setLicense(null))
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-screen bg-gray-950">
        <div className="flex flex-col items-center gap-3">
          <div className="w-8 h-8 border-2 border-forge-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-gray-400 text-sm">Loading...</span>
        </div>
      </div>
    );
  }

  if (!license) {
    return <ActivationPage onActivated={setLicense} />;
  }

  return (
    <div className="flex h-screen bg-gray-950">
      <Sidebar license={license} />
      <main className="flex-1 overflow-y-auto p-6">
        <Routes>
          <Route path="/" element={<Navigate to="/catalog" replace />} />
          <Route path="/catalog" element={<CatalogPage license={license} />} />
          <Route path="/installed" element={<InstalledPage />} />
          <Route path="/settings" element={<SettingsPage license={license} onDeactivated={() => setLicense(null)} />} />
          <Route path="/feedback" element={<FeedbackPage />} />
        </Routes>
      </main>
    </div>
  );
}

export default App;

import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface Package {
  name: string;
  status: string;
}

interface SearchResult {
  name: string;
  description: string;
}

export default function PackagesPanel() {
  const { t } = useT();
  const { data: installed, loading, error, refetch } = useApi<Package[]>('/api/packages/installed');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [installName, setInstallName] = useState('');
  const [message, setMessage] = useState('');
  const [actionLoading, setActionLoading] = useState(false);
  const [filter, setFilter] = useState('');

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!searchQuery.trim()) return;
    setSearching(true);
    const res = await apiPost<SearchResult[]>('/api/packages/search', { query: searchQuery });
    if (res.success && res.data) setSearchResults(res.data);
    else setMessage(res.message);
    setSearching(false);
  };

  const handleInstall = async (name: string) => {
    if (!window.confirm(`Install "${name}"?`)) return;
    setActionLoading(true);
    setMessage('');
    const res = await apiPost<string>('/api/packages/install', { name });
    setMessage(res.message || (res.data as string));
    setActionLoading(false);
    if (res.success) refetch();
  };

  const handleRemove = async (name: string) => {
    if (!window.confirm(`Remove "${name}"?`)) return;
    setActionLoading(true);
    setMessage('');
    const res = await apiPost<string>('/api/packages/remove', { name });
    setMessage(res.message || (res.data as string));
    setActionLoading(false);
    if (res.success) refetch();
  };

  const filteredPackages = (installed || []).filter(
    p => p.name.toLowerCase().includes(filter.toLowerCase())
  );

  if (loading) return <div className="panel loading">{t('packages.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <h2>{t('packages.title')}</h2>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      <div className="inline-form" style={{ flexDirection: 'row', alignItems: 'center' }}>
        <input
          placeholder={t('packages.installPlaceholder')}
          value={installName}
          onChange={e => setInstallName(e.target.value)}
          style={{ flex: 1 }}
        />
        <button
          className="btn btn-primary"
          disabled={actionLoading || !installName.trim()}
          onClick={() => handleInstall(installName.trim())}
        >
          {actionLoading ? t('common.working') : t('common.install')}
        </button>
      </div>

      <form onSubmit={handleSearch} className="inline-form" style={{ flexDirection: 'row', alignItems: 'center' }}>
        <input
          placeholder={t('packages.searchPlaceholder')}
          value={searchQuery}
          onChange={e => setSearchQuery(e.target.value)}
          style={{ flex: 1 }}
        />
        <button type="submit" className="btn btn-secondary" disabled={searching}>
          {searching ? t('packages.searching') : t('common.search')}
        </button>
      </form>

      {searchResults.length > 0 && (
        <>
          <h3>{t('packages.searchResults')}</h3>
          <table className="data-table">
            <thead>
              <tr><th>{t('packages.package')}</th><th>{t('common.description')}</th><th>{t('packages.action')}</th></tr>
            </thead>
            <tbody>
              {searchResults.map((r, i) => (
                <tr key={i}>
                  <td><strong>{r.name}</strong></td>
                  <td>{r.description}</td>
                  <td>
                    <button className="btn btn-primary btn-sm"
                      disabled={actionLoading}
                      onClick={() => handleInstall(r.name)}>{t('common.install')}</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      <h3>{t('packages.installed')} ({filteredPackages.length})</h3>
      <input
        placeholder={t('packages.filterPlaceholder')}
        value={filter}
        onChange={e => setFilter(e.target.value)}
        className="filter-input"
      />
      <div className="package-list">
        <table className="data-table">
          <thead>
            <tr><th>{t('packages.package')}</th><th>{t('common.status')}</th><th>{t('packages.action')}</th></tr>
          </thead>
          <tbody>
            {filteredPackages.slice(0, 100).map((pkg, i) => (
              <tr key={i}>
                <td>{pkg.name}</td>
                <td><code>{pkg.status}</code></td>
                <td>
                  <button className="btn btn-danger btn-sm"
                    disabled={actionLoading}
                    onClick={() => handleRemove(pkg.name)}>{t('common.remove')}</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {filteredPackages.length > 100 && (
          <p className="text-secondary">{t('common.showingFirst', { count: filteredPackages.length })}</p>
        )}
      </div>
    </div>
  );
}

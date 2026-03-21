import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';

interface Package {
  name: string;
  status: string;
}

interface SearchResult {
  name: string;
  description: string;
}

export default function PackagesPanel() {
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

  if (loading) return <div className="panel loading">Loading packages...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;

  return (
    <div className="panel">
      <h2>Package Management</h2>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {/* Direct install */}
      <div className="inline-form" style={{ flexDirection: 'row', alignItems: 'center' }}>
        <input
          placeholder="Package name to install"
          value={installName}
          onChange={e => setInstallName(e.target.value)}
          style={{ flex: 1 }}
        />
        <button
          className="btn btn-primary"
          disabled={actionLoading || !installName.trim()}
          onClick={() => handleInstall(installName.trim())}
        >
          {actionLoading ? 'Working...' : 'Install'}
        </button>
      </div>

      {/* Search */}
      <form onSubmit={handleSearch} className="inline-form" style={{ flexDirection: 'row', alignItems: 'center' }}>
        <input
          placeholder="Search packages..."
          value={searchQuery}
          onChange={e => setSearchQuery(e.target.value)}
          style={{ flex: 1 }}
        />
        <button type="submit" className="btn btn-secondary" disabled={searching}>
          {searching ? 'Searching...' : 'Search'}
        </button>
      </form>

      {searchResults.length > 0 && (
        <>
          <h3>Search Results</h3>
          <table className="data-table">
            <thead>
              <tr><th>Package</th><th>Description</th><th>Action</th></tr>
            </thead>
            <tbody>
              {searchResults.map((r, i) => (
                <tr key={i}>
                  <td><strong>{r.name}</strong></td>
                  <td>{r.description}</td>
                  <td>
                    <button className="btn btn-primary btn-sm"
                      disabled={actionLoading}
                      onClick={() => handleInstall(r.name)}>Install</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {/* Installed packages */}
      <h3>Installed Packages ({filteredPackages.length})</h3>
      <input
        placeholder="Filter installed packages..."
        value={filter}
        onChange={e => setFilter(e.target.value)}
        className="filter-input"
      />
      <div className="package-list">
        <table className="data-table">
          <thead>
            <tr><th>Package</th><th>Status</th><th>Action</th></tr>
          </thead>
          <tbody>
            {filteredPackages.slice(0, 100).map((pkg, i) => (
              <tr key={i}>
                <td>{pkg.name}</td>
                <td><code>{pkg.status}</code></td>
                <td>
                  <button className="btn btn-danger btn-sm"
                    disabled={actionLoading}
                    onClick={() => handleRemove(pkg.name)}>Remove</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {filteredPackages.length > 100 && (
          <p className="text-secondary">Showing first 100 of {filteredPackages.length} packages. Use filter to narrow.</p>
        )}
      </div>
    </div>
  );
}

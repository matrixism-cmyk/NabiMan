import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { UpdateCheckResult } from '../types';
import { useT } from '../i18n';

export default function UpdatesPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<UpdateCheckResult>('/api/updates/check');
  const [upgrading, setUpgrading] = useState(false);
  const [upgradingPkg, setUpgradingPkg] = useState('');
  const [result, setResult] = useState('');

  const handleUpgradeAll = async () => {
    if (!window.confirm(t('updates.confirmUpgrade'))) return;
    setUpgrading(true); setResult('');
    const res = await apiPost<string>('/api/updates/upgrade', {});
    setResult(res.data || res.message);
    setUpgrading(false); refetch();
  };

  const handleUpgradeOne = async (name: string) => {
    setUpgradingPkg(name); setResult('');
    const res = await apiPost<string>('/api/updates/upgrade-package', { name });
    setResult(res.data || res.message);
    setUpgradingPkg(''); refetch();
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('updates.title')}</h2>
        <div className="btn-group">
          <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('updates.checkAgain')}</button>
          {data.count > 0 && (
            <button className="btn btn-primary btn-sm" onClick={handleUpgradeAll} disabled={upgrading}>
              {upgrading ? t('common.working') : t('updates.upgradeAll')} ({data.count})
            </button>
          )}
        </div>
      </div>
      {data.count === 0 ? (
        <p className="text-success">{t('updates.upToDate')}</p>
      ) : (
        <table className="data-table">
          <thead><tr>
            <th>{t('updates.package')}</th>
            <th>{t('updates.current')}</th>
            <th>{t('updates.available')}</th>
            <th>{t('common.actions')}</th>
          </tr></thead>
          <tbody>
            {data.packages.map((pkg, i) => (
              <tr key={i}>
                <td><strong>{pkg.name}</strong></td>
                <td><code>{pkg.current_version}</code></td>
                <td><code>{pkg.new_version}</code></td>
                <td>
                  <button className="btn btn-sm btn-primary"
                    onClick={() => handleUpgradeOne(pkg.name)}
                    disabled={upgradingPkg === pkg.name || upgrading}>
                    {upgradingPkg === pkg.name ? t('common.working') : t('updates.upgrade')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {result && <pre className="log-output">{result}</pre>}
    </div>
  );
}

import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { SystemService } from '../types';
import { useT } from '../i18n';

export default function ServicesPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<SystemService[]>('/api/services', 10000);
  const [filter, setFilter] = useState('');
  const [stateFilter, setStateFilter] = useState<'all' | 'active' | 'inactive' | 'failed'>('all');
  const [message, setMessage] = useState('');
  const [actionLoading, setActionLoading] = useState('');

  const handleAction = async (name: string, action: string) => {
    if ((action === 'stop' || action === 'disable') && !window.confirm(`${action} "${name}"?`)) return;
    setActionLoading(name);
    setMessage('');
    const res = await apiPost<string>('/api/services/action', { name, action });
    setMessage(res.success ? res.data || `${action} OK` : res.message);
    setActionLoading('');
    refetch();
  };

  if (loading) return <div className="panel loading">{t('services.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  const filtered = (data || []).filter(s => {
    const matchName = s.name.toLowerCase().includes(filter.toLowerCase())
      || s.description.toLowerCase().includes(filter.toLowerCase());
    const matchState = stateFilter === 'all'
      || (stateFilter === 'active' && s.active_state === 'active')
      || (stateFilter === 'inactive' && s.active_state === 'inactive')
      || (stateFilter === 'failed' && s.active_state === 'failed');
    return matchName && matchState;
  });

  const stateButtons: { key: typeof stateFilter; labelKey: string }[] = [
    { key: 'all', labelKey: 'common.all' },
    { key: 'active', labelKey: 'services.active' },
    { key: 'inactive', labelKey: 'services.inactive' },
    { key: 'failed', labelKey: 'services.failed' },
  ];

  return (
    <div className="panel">
      <h2>{t('services.title')}</h2>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      <div className="filter-row">
        <input
          placeholder={t('services.filterPlaceholder')}
          value={filter}
          onChange={e => setFilter(e.target.value)}
          className="filter-input"
        />
        <div className="btn-group">
          {stateButtons.map(s => (
            <button key={s.key} className={`btn btn-sm ${stateFilter === s.key ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setStateFilter(s.key)}>
              {t(s.labelKey)}
            </button>
          ))}
        </div>
      </div>

      <p className="text-secondary">{filtered.length} {t('services.services')}</p>

      <table className="data-table">
        <thead>
          <tr>
            <th>{t('services.service')}</th>
            <th>{t('common.description')}</th>
            <th>{t('services.state')}</th>
            <th>{t('services.enabled')}</th>
            <th>{t('common.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {filtered.slice(0, 100).map(svc => (
            <tr key={svc.name}>
              <td><strong>{svc.name}</strong></td>
              <td className="text-secondary">{svc.description}</td>
              <td>
                <span className={`status-badge ${
                  svc.active_state === 'active' ? 'up' :
                  svc.active_state === 'failed' ? 'error' : 'down'
                }`}>
                  {svc.active_state} ({svc.sub_state})
                </span>
              </td>
              <td>
                <span className={svc.enabled ? 'text-success' : 'text-secondary'}>
                  {svc.enabled ? t('common.yes') : t('common.no')}
                </span>
              </td>
              <td>
                <div className="btn-group">
                  {svc.active_state !== 'active' && (
                    <button className="btn btn-primary btn-sm"
                      disabled={actionLoading === svc.name}
                      onClick={() => handleAction(svc.name, 'start')}>{t('common.start')}</button>
                  )}
                  {svc.active_state === 'active' && (
                    <button className="btn btn-warning btn-sm"
                      disabled={actionLoading === svc.name}
                      onClick={() => handleAction(svc.name, 'stop')}>{t('common.stop')}</button>
                  )}
                  <button className="btn btn-secondary btn-sm"
                    disabled={actionLoading === svc.name}
                    onClick={() => handleAction(svc.name, 'restart')}>{t('common.restart')}</button>
                  {!svc.enabled ? (
                    <button className="btn btn-secondary btn-sm"
                      disabled={actionLoading === svc.name}
                      onClick={() => handleAction(svc.name, 'enable')}>{t('services.enable')}</button>
                  ) : (
                    <button className="btn btn-secondary btn-sm"
                      disabled={actionLoading === svc.name}
                      onClick={() => handleAction(svc.name, 'disable')}>{t('services.disable')}</button>
                  )}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {filtered.length > 100 && (
        <p className="text-secondary">{t('common.showingFirst', { count: filtered.length })}</p>
      )}
    </div>
  );
}

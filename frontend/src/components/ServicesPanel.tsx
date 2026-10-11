import React, { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { SystemService } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';
import { useServiceControl } from '../hooks/useServiceControl';
import ServiceControlResult from './ServiceControlResult';

export default function ServicesPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<SystemService[]>('/api/services', 10000);
  const [filter, setFilter] = useState('');
  const [stateFilter, setStateFilter] = useState<'all' | 'active' | 'inactive' | 'failed'>('all');
  const control = useServiceControl(refetch);

  const filtered = (data || []).filter(s => {
    const matchName = s.name.toLowerCase().includes(filter.toLowerCase())
      || s.description.toLowerCase().includes(filter.toLowerCase());
    const matchState = stateFilter === 'all'
      || (stateFilter === 'active' && s.active_state === 'active')
      || (stateFilter === 'inactive' && s.active_state === 'inactive')
      || (stateFilter === 'failed' && s.active_state === 'failed');
    return matchName && matchState;
  });

  const { sorted, toggle, indicator } = useSortable(filtered, 'name', 'asc');
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );


  const stateButtons: { key: typeof stateFilter; labelKey: string }[] = [
    { key: 'all', labelKey: 'common.all' },
    { key: 'active', labelKey: 'services.active' },
    { key: 'inactive', labelKey: 'services.inactive' },
    { key: 'failed', labelKey: 'services.failed' },
  ];

  return (
    <div className="panel">
      <h2>{t('services.title')}</h2>

      {loading && <p>{t('services.loading')}</p>}
      {error && <div className="panel error" role="alert">{t('common.error')}: {error}</div>}
      {control.message && <div className="message" role="status">{control.message}</div>}
      <ServiceControlResult operationId={control.operationId} setOperationId={control.setOperationId}
        result={control.result} busy={control.busy} inspect={control.inspect} />

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

      {!loading && !error && <p className="text-secondary">{filtered.length} {t('services.services')}</p>}

      {!loading && !error && <table className="data-table">
        <thead>
          <tr>
            {S('name', t('services.service'))}
            <th>{t('common.description')}</th>
            {S('active_state', t('services.state'))}
            {S('enabled', t('services.enabled'))}
            <th>{t('common.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {sorted.slice(0, 100).map(svc => (
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
                      disabled={control.busy || !control.ready}
                      onClick={() => control.act(svc.name, 'start')}>{t('common.start')}</button>
                  )}
                  {svc.active_state === 'active' && (
                    <button className="btn btn-warning btn-sm"
                      disabled={control.busy || !control.ready}
                      onClick={() => control.act(svc.name, 'stop')}>{t('common.stop')}</button>
                  )}
                  <button className="btn btn-secondary btn-sm"
                    disabled={control.busy || !control.ready}
                    onClick={() => control.act(svc.name, 'restart')}>{t('common.restart')}</button>
                  {!svc.enabled ? (
                    <button className="btn btn-secondary btn-sm"
                      disabled={control.busy || !control.ready}
                      onClick={() => control.act(svc.name, 'enable')}>{t('services.enable')}</button>
                  ) : (
                    <button className="btn btn-secondary btn-sm"
                      disabled={control.busy || !control.ready}
                      onClick={() => control.act(svc.name, 'disable')}>{t('services.disable')}</button>
                  )}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>}
      {!loading && !error && filtered.length > 100 && (
        <p className="text-secondary">{t('common.showingFirst', { count: filtered.length })}</p>
      )}
    </div>
  );
}

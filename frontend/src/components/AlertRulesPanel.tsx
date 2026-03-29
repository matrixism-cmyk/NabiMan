import React, { useState } from 'react';
import { useApi, apiPost, apiRequest } from '../hooks/useApi';
import { useT } from '../i18n';

interface AlertRule { id: string; name: string; metric: string; threshold: number; duration_secs: number; enabled: boolean; last_triggered: string | null; service_name?: string; }

export default function AlertRulesPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<AlertRule[]>('/api/alerts/rules');
  const [showAdd, setShowAdd] = useState(false);
  const [name, setName] = useState('');
  const [metric, setMetric] = useState('cpu');
  const [threshold, setThreshold] = useState('80');
  const [cooldown, setCooldown] = useState('300');
  const [serviceName, setServiceName] = useState('');
  const [msg, setMsg] = useState('');

  const addRule = async () => {
    if (!name) return;
    if (metric === 'service_down' && !serviceName.trim()) return;
    const body: Record<string, unknown> = {
      id: '', name, metric,
      threshold: metric === 'service_down' ? 0 : parseFloat(threshold),
      duration_secs: parseInt(cooldown), enabled: true, last_triggered: null,
    };
    if (metric === 'service_down') body.service_name = serviceName.trim();
    const res = await apiPost<string>('/api/alerts/rules', body);
    setMsg(res.data || res.message);
    if (res.success) { setShowAdd(false); setName(''); setServiceName(''); refetch(); }
  };

  const deleteRule = async (id: string) => {
    if (!window.confirm(t('alerts.confirmDelete'))) return;
    await apiRequest<string>(`/api/alerts/rules/${id}`, { method: 'DELETE' });
    refetch();
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('alerts.title')}</h2>
        <button className="btn btn-primary btn-sm" onClick={() => setShowAdd(!showAdd)}>
          {showAdd ? t('common.cancel') : t('alerts.addRule')}
        </button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {showAdd && (
        <div className="inline-form">
          <div className="form-row">
            <label>{t('common.name')}</label>
            <input value={name} onChange={e => setName(e.target.value)} placeholder="CPU High Alert" />
          </div>
          <div className="form-row">
            <label>{t('alerts.metric')}</label>
            <select className="select-input" value={metric} onChange={e => setMetric(e.target.value)}>
              <option value="cpu">CPU</option>
              <option value="memory">{t('charts.memory')}</option>
              <option value="disk">{t('serverStatus.disk')}</option>
              <option value="service_down">{t('alerts.serviceDown')}</option>
            </select>
          </div>
          {metric === 'service_down' ? (
            <div className="form-row">
              <label>{t('alerts.serviceName')}</label>
              <input value={serviceName} onChange={e => setServiceName(e.target.value)} placeholder="nginx" />
            </div>
          ) : (
            <div className="form-row">
              <label>{t('alerts.threshold')}</label>
              <input type="number" value={threshold} onChange={e => setThreshold(e.target.value)} style={{ width: 80 }} />
              <span>%</span>
            </div>
          )}
          <div className="form-row">
            <label>{t('alerts.cooldown')}</label>
            <input type="number" value={cooldown} onChange={e => setCooldown(e.target.value)} style={{ width: 80 }} />
            <span>sec</span>
          </div>
          <button className="btn btn-primary" onClick={addRule}>{t('common.add')}</button>
        </div>
      )}
      {data && data.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>{t('common.name')}</th><th>{t('alerts.metric')}</th><th>{t('alerts.threshold')}</th><th>{t('alerts.cooldown')}</th><th>{t('alerts.lastTriggered')}</th><th>{t('common.actions')}</th></tr></thead>
          <tbody>
            {data.map(r => (
              <tr key={r.id}>
                <td><strong>{r.name}</strong></td>
                <td><code>{r.metric}</code></td>
                <td>{r.metric === 'service_down' ? (r.service_name || '-') : `${r.threshold}%`}</td>
                <td>{r.duration_secs}s</td>
                <td>{r.last_triggered || '-'}</td>
                <td><button className="btn btn-sm btn-danger" onClick={() => deleteRule(r.id)}>{t('common.delete')}</button></td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p className="text-secondary">{t('alerts.noRules')}</p>}
    </div>
  );
}

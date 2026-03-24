import React, { useState } from 'react';
import { useApi, apiPost, apiRequest } from '../hooks/useApi';
import { useT } from '../i18n';

interface Channel { id: string; name: string; channel_type: string; target: string; enabled: boolean; }

export default function NotificationsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<Channel[]>('/api/notifications/channels');
  const [showAdd, setShowAdd] = useState(false);
  const [name, setName] = useState('');
  const [type_, setType] = useState('webhook');
  const [target, setTarget] = useState('');
  const [msg, setMsg] = useState('');

  const addChannel = async () => {
    if (!name || !target) return;
    const res = await apiPost<string>('/api/notifications/channels', { name, channel_type: type_, target });
    setMsg(res.data || res.message);
    if (res.success) { setShowAdd(false); setName(''); setTarget(''); refetch(); }
  };

  const deleteChannel = async (id: string) => {
    if (!window.confirm(t('notify.confirmDelete'))) return;
    await apiRequest<string>(`/api/notifications/channels/${id}`, { method: 'DELETE' });
    refetch();
  };

  const testAll = async () => {
    const res = await apiPost<string>('/api/notifications/test', {});
    setMsg(res.data || res.message);
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('notify.title')}</h2>
        <div className="btn-group">
          <button className="btn btn-primary btn-sm" onClick={() => setShowAdd(!showAdd)}>
            {showAdd ? t('common.cancel') : t('notify.addChannel')}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={testAll}>{t('notify.test')}</button>
        </div>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {showAdd && (
        <div className="inline-form">
          <div className="form-row">
            <label>{t('common.name')}</label>
            <input value={name} onChange={e => setName(e.target.value)} placeholder="Slack Alert" />
          </div>
          <div className="form-row">
            <label>{t('notify.type')}</label>
            <select className="select-input" value={type_} onChange={e => setType(e.target.value)}>
              <option value="webhook">Webhook</option>
              <option value="slack">Slack</option>
              <option value="email">Email</option>
            </select>
          </div>
          <div className="form-row">
            <label>{t('notify.target')}</label>
            <input value={target} onChange={e => setTarget(e.target.value)} placeholder="URL or email" />
          </div>
          <button className="btn btn-primary" onClick={addChannel}>{t('common.add')}</button>
        </div>
      )}
      {data && data.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>{t('common.name')}</th><th>{t('notify.type')}</th><th>{t('notify.target')}</th><th>{t('common.actions')}</th></tr></thead>
          <tbody>
            {data.map(ch => (
              <tr key={ch.id}>
                <td><strong>{ch.name}</strong></td>
                <td><code>{ch.channel_type}</code></td>
                <td style={{ maxWidth: 300, overflow: 'hidden', textOverflow: 'ellipsis' }}>{ch.target}</td>
                <td><button className="btn btn-sm btn-danger" onClick={() => deleteChannel(ch.id)}>{t('common.delete')}</button></td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p className="text-secondary">{t('notify.noChannels')}</p>}
    </div>
  );
}

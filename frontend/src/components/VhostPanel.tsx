import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface VhostInfo { domain: string; vhost_type: string; target: string; ssl: boolean; enabled: boolean; config_file: string; }
interface VhostConfig { domain: string; http_config: string; ssl_config: string | null; }

export default function VhostPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<VhostInfo[]>('/api/vhost');
  const [showAdd, setShowAdd] = useState(false);
  const [domain, setDomain] = useState('');
  const [vtype, setVtype] = useState('static');
  const [target, setTarget] = useState('');
  const [wsPath, setWsPath] = useState('');
  const [autoSsl, setAutoSsl] = useState(true);
  const [msg, setMsg] = useState('');
  const [creating, setCreating] = useState(false);
  // Edit state
  const [editing, setEditing] = useState<VhostConfig | null>(null);
  const [editHttp, setEditHttp] = useState('');
  const [editSsl, setEditSsl] = useState('');
  const [saving, setSaving] = useState(false);

  const handleCreate = async () => {
    if (!domain || !target) return;
    setCreating(true); setMsg('');
    const res = await apiPost<string>('/api/vhost', {
      domain, vhost_type: vtype, target, auto_ssl: autoSsl,
      websocket_path: wsPath || null,
    });
    setMsg(res.data || res.message);
    setCreating(false);
    if (res.success) { setShowAdd(false); setDomain(''); setTarget(''); refetch(); }
  };

  const handleDelete = async (d: string) => {
    if (!window.confirm(t('vhost.confirmDelete').replace('{domain}', d))) return;
    const res = await apiPost<string>('/api/vhost/delete', { domain: d });
    setMsg(res.data || res.message); refetch();
  };

  const handleEdit = async (d: string) => {
    const res = await fetch(`/api/vhost/config/${d}`, {
      headers: { 'Authorization': `Bearer ${sessionStorage.getItem('nabiman_token')}` },
    });
    const json = await res.json();
    if (json.success && json.data) {
      setEditing(json.data);
      setEditHttp(json.data.http_config);
      setEditSsl(json.data.ssl_config || '');
    } else { setMsg(json.message); }
  };

  const handleSave = async () => {
    if (!editing) return;
    setSaving(true);
    const res = await apiPost<string>('/api/vhost/config', {
      domain: editing.domain, http_config: editHttp,
      ssl_config: editSsl || null,
    });
    setMsg(res.data || res.message);
    setSaving(false);
    if (res.success) { setEditing(null); refetch(); }
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  const typeLabel = (vt: string) => {
    if (vt === 'proxy') return t('vhost.proxy');
    if (vt === 'redirect') return t('vhost.redirect');
    return t('vhost.static');
  };

  // Edit view
  if (editing) return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('vhost.editTitle')} - {editing.domain}</h2>
        <div className="btn-group">
          <button className="btn btn-primary btn-sm" onClick={handleSave} disabled={saving}>
            {saving ? t('common.working') : t('common.save')}
          </button>
          <button className="btn btn-secondary btn-sm" onClick={() => setEditing(null)}>{t('common.cancel')}</button>
        </div>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      <h3>HTTP {t('vhost.config')}</h3>
      <textarea className="file-editor" value={editHttp} onChange={e => setEditHttp(e.target.value)} />
      {editSsl && (<>
        <h3>SSL {t('vhost.config')}</h3>
        <textarea className="file-editor" value={editSsl} onChange={e => setEditSsl(e.target.value)} />
      </>)}
    </div>
  );

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('vhost.title')}</h2>
        <button className="btn btn-primary btn-sm" onClick={() => setShowAdd(!showAdd)}>
          {showAdd ? t('common.cancel') : t('vhost.add')}
        </button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {showAdd && (
        <div className="inline-form">
          <div className="form-row">
            <label>{t('vhost.domain')}</label>
            <input value={domain} onChange={e => setDomain(e.target.value)} placeholder="example.com" />
          </div>
          <div className="form-row">
            <label>{t('vhost.type')}</label>
            <select className="select-input" value={vtype} onChange={e => setVtype(e.target.value)}>
              <option value="static">{t('vhost.static')} (DocumentRoot)</option>
              <option value="proxy">{t('vhost.proxy')} (Reverse Proxy)</option>
              <option value="redirect">{t('vhost.redirect')} (301 Redirect)</option>
            </select>
          </div>
          <div className="form-row">
            <label>{vtype === 'static' ? t('vhost.docroot') : vtype === 'proxy' ? t('vhost.proxyUrl') : t('vhost.redirectUrl')}</label>
            <input value={target} onChange={e => setTarget(e.target.value)}
              placeholder={vtype === 'static' ? '/var/www/example' : vtype === 'proxy' ? 'http://127.0.0.1:3000/' : 'https://other.com/'} />
          </div>
          {vtype === 'proxy' && (
            <div className="form-row">
              <label>WebSocket</label>
              <input value={wsPath} onChange={e => setWsPath(e.target.value)} placeholder={t('vhost.wsPlaceholder')} />
            </div>
          )}
          <div className="form-row">
            <label>SSL</label>
            <label style={{ display: 'flex', alignItems: 'center', gap: 6, minWidth: 'auto' }}>
              <input type="checkbox" checked={autoSsl} onChange={e => setAutoSsl(e.target.checked)} />
              {t('vhost.autoSsl')}
            </label>
          </div>
          <button className="btn btn-primary" onClick={handleCreate} disabled={creating}>
            {creating ? t('common.working') : t('vhost.create')}
          </button>
        </div>
      )}
      {data && data.length > 0 ? (
        <table className="data-table">
          <thead><tr>
            <th>{t('vhost.domain')}</th><th>{t('vhost.type')}</th><th>{t('vhost.target')}</th>
            <th>SSL</th><th>{t('common.actions')}</th>
          </tr></thead>
          <tbody>
            {data.map((v, i) => (
              <tr key={i}>
                <td><strong>{v.domain}</strong></td>
                <td><code>{typeLabel(v.vhost_type)}</code></td>
                <td style={{ maxWidth: 250, overflow: 'hidden', textOverflow: 'ellipsis' }}>{v.target}</td>
                <td><span className={`status-badge ${v.ssl ? 'up' : 'down'}`}>{v.ssl ? 'HTTPS' : 'HTTP'}</span></td>
                <td className="btn-group">
                  <button className="btn btn-sm btn-secondary" onClick={() => handleEdit(v.domain)}>{t('common.edit')}</button>
                  <button className="btn btn-sm btn-danger" onClick={() => handleDelete(v.domain)}>{t('common.delete')}</button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : <p className="text-secondary">{t('vhost.noVhosts')}</p>}
    </div>
  );
}

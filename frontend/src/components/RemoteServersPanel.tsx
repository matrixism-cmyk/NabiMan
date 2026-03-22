import React, { useState } from 'react';
import { useApi, apiPost, apiRequest } from '../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../types';
import { useT } from '../i18n';
import { SshKeyPanel, DeployKeyDialog } from './SshKeyComponents';

function AddServerForm({ onAdded }: { onAdded: () => void }) {
  const { t } = useT();
  const [name, setName] = useState('');
  const [host, setHost] = useState('');
  const [port, setPort] = useState('22');
  const [user, setUser] = useState('root');
  const [authMethod, setAuthMethod] = useState('key');
  const [tags, setTags] = useState('');
  const [memo, setMemo] = useState('');
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !host.trim()) return;
    setSaving(true);
    const res = await apiPost<RemoteServer>('/api/remote-servers', {
      name: name.trim(), host: host.trim(), port: parseInt(port) || 22,
      user: user.trim() || 'root', auth_method: authMethod,
      tags: tags ? tags.split(',').map(t => t.trim()).filter(Boolean) : [],
      memo: memo.trim(),
    });
    setMessage(res.success ? t('remote.serverAdded') : res.message);
    if (res.success) { setName(''); setHost(''); setPort('22'); setUser('root'); setTags(''); setMemo(''); onAdded(); }
    setSaving(false);
  };

  return (
    <form onSubmit={handleSubmit} className="add-server-form">
      <h3>{t('remote.addRemoteServer')}</h3>
      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}
      <div className="form-grid">
        <div className="form-group"><label>{t('common.name')} *</label>
          <input value={name} onChange={e => setName(e.target.value)} placeholder="Production Web" required className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.host')} *</label>
          <input value={host} onChange={e => setHost(e.target.value)} placeholder="192.168.1.100" required className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.port')}</label>
          <input value={port} onChange={e => setPort(e.target.value)} type="number" className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.user')}</label>
          <input value={user} onChange={e => setUser(e.target.value)} placeholder="root" className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.auth')}</label>
          <select value={authMethod} onChange={e => setAuthMethod(e.target.value)} className="select-input">
            <option value="key">{t('remote.sshKey')}</option><option value="password">{t('accounts.password')}</option>
          </select></div>
        <div className="form-group"><label>{t('remote.tags')}</label>
          <input value={tags} onChange={e => setTags(e.target.value)} placeholder="web, production" className="filter-input" /></div>
        <div className="form-group" style={{ gridColumn: '1 / -1' }}><label>{t('remote.memo')}</label>
          <input value={memo} onChange={e => setMemo(e.target.value)} placeholder="..." className="filter-input" /></div>
      </div>
      <button type="submit" className="btn btn-primary" disabled={saving}>
        {saving ? t('remote.adding') : t('remote.addServer')}</button>
    </form>
  );
}

function ServerCard({ server, onRefresh, onConnectSSH }: { server: RemoteServer; onRefresh: () => void; onConnectSSH: (server: RemoteServer) => void }) {
  const { t } = useT();
  const [status, setStatus] = useState<RemoteServerStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [execCmd, setExecCmd] = useState('');
  const [execResult, setExecResult] = useState('');
  const [executing, setExecuting] = useState(false);
  const [showExec, setShowExec] = useState(false);
  const [showDeploy, setShowDeploy] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState(server.name);
  const [editMemo, setEditMemo] = useState(server.memo);
  const [editTags, setEditTags] = useState(server.tags.join(', '));

  const handleCheck = async () => { setChecking(true); const res = await apiPost<RemoteServerStatus>(`/api/remote-servers/${server.id}/check`, {}); if (res.success && res.data) setStatus(res.data); setChecking(false); };
  const handleDelete = async () => { if (!window.confirm(`Delete "${server.name}"?`)) return; await apiRequest(`/api/remote-servers/${server.id}`, { method: 'DELETE' }); onRefresh(); };
  const handleExec = async () => { if (!execCmd.trim()) return; setExecuting(true); const res = await apiPost<string>(`/api/remote-servers/${server.id}/exec`, { command: execCmd }); setExecResult(res.success ? (res.data || '') : `Error: ${res.message}`); setExecuting(false); };
  const handleSaveEdit = async () => { await apiRequest(`/api/remote-servers/${server.id}`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ name: editName, memo: editMemo, tags: editTags.split(',').map(t => t.trim()).filter(Boolean) }) }); setEditing(false); onRefresh(); };

  const statusColor = server.status === 'online' ? 'up' : server.status === 'offline' ? 'down' : '';

  return (
    <div className="config-section">
      <div className="config-header">
        {editing ? (
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flex: 1 }}>
            <input value={editName} onChange={e => setEditName(e.target.value)} className="filter-input" style={{ width: '200px' }} />
            <input value={editTags} onChange={e => setEditTags(e.target.value)} className="filter-input" placeholder={t('remote.tags')} style={{ width: '200px' }} />
            <input value={editMemo} onChange={e => setEditMemo(e.target.value)} className="filter-input" placeholder={t('remote.memo')} style={{ flex: 1 }} />
            <button className="btn btn-primary btn-sm" onClick={handleSaveEdit}>{t('common.save')}</button>
            <button className="btn btn-secondary btn-sm" onClick={() => setEditing(false)}>{t('common.cancel')}</button>
          </div>
        ) : (
          <>
            <h3>{server.name}</h3>
            <div className="config-meta">
              <span className={`status-badge ${statusColor}`}>{server.status || t('common.unknown')}</span>
              <code>{server.user}@{server.host}:{server.port}</code>
              <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{server.auth_method === 'key' ? t('remote.sshKey') : t('accounts.password')}</span>
              {server.tags.map(tag => <span key={tag} className="tag-badge">{tag}</span>)}
            </div>
            <div className="btn-group">
              <button className="btn btn-primary btn-sm" onClick={handleCheck} disabled={checking}>{checking ? t('remote.checking') : t('remote.check')}</button>
              <button className="btn btn-secondary btn-sm" onClick={() => onConnectSSH(server)}>SSH</button>
              <button className="btn btn-secondary btn-sm" onClick={() => setShowDeploy(!showDeploy)}>{showDeploy ? t('remote.hideKey') : t('remote.deployKey')}</button>
              <button className="btn btn-secondary btn-sm" onClick={() => setShowExec(!showExec)}>{t('remote.exec')}</button>
              <button className="btn btn-secondary btn-sm" onClick={() => { setEditing(true); setEditName(server.name); setEditMemo(server.memo); setEditTags(server.tags.join(', ')); }}>{t('common.edit')}</button>
              <button className="btn btn-danger btn-sm" onClick={handleDelete}>{t('common.delete')}</button>
            </div>
          </>
        )}
      </div>
      {server.memo && !editing && <div style={{ fontSize: '12px', color: 'var(--text-secondary)', padding: '2px 0' }}>{server.memo}</div>}
      {showDeploy && <DeployKeyDialog server={server} onClose={() => setShowDeploy(false)} />}
      {status && (
        <div className="server-status-grid">
          <div className="status-item"><label>{t('remote.hostname')}</label><span>{status.hostname}</span></div>
          <div className="status-item"><label>{t('remote.os')}</label><span>{status.os}</span></div>
          <div className="status-item"><label>{t('remote.uptime')}</label><span>{status.uptime}</span></div>
          <div className="status-item"><label>{t('remote.cpu')}</label><span>{status.cpu_usage}</span></div>
          <div className="status-item"><label>{t('remote.memory')}</label><span>{status.memory}</span></div>
          <div className="status-item"><label>{t('remote.diskRoot')}</label><span>{status.disk}</span></div>
          <div className="status-item"><label>{t('remote.load')}</label><span>{status.load}</span></div>
          <div className="status-item"><label>{t('remote.checked')}</label><span>{status.checked_at}</span></div>
        </div>
      )}
      {showExec && (
        <div className="exec-area">
          <div className="filter-row">
            <input value={execCmd} onChange={e => setExecCmd(e.target.value)} placeholder={t('remote.execPlaceholder')} className="filter-input" style={{ flex: 1 }} onKeyDown={e => e.key === 'Enter' && handleExec()} />
            <button className="btn btn-primary btn-sm" onClick={handleExec} disabled={executing}>{executing ? t('common.running') : t('common.run')}</button>
          </div>
          {execResult && <pre className="config-preview" style={{ maxHeight: '200px' }}>{execResult}</pre>}
        </div>
      )}
      {server.last_checked && !status && <div style={{ fontSize: '11px', color: 'var(--text-secondary)', padding: '2px 0' }}>{t('remote.lastChecked')}: {server.last_checked}</div>}
    </div>
  );
}

export default function RemoteServersPanel({ onConnectSSH }: { onConnectSSH?: (host: string, port: number, user: string) => void }) {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<RemoteServer[]>('/api/remote-servers');
  const [showAdd, setShowAdd] = useState(false);
  const [showKeyPanel, setShowKeyPanel] = useState(false);
  const [filter, setFilter] = useState('');
  const [checkingAll, setCheckingAll] = useState(false);
  const [message, setMessage] = useState('');

  const handleCheckAll = async () => {
    setCheckingAll(true); setMessage(t('remote.checkingAll'));
    const res = await apiPost<RemoteServerStatus[]>('/api/remote-servers/check-all', {});
    if (res.success && res.data) { setMessage(`${res.data.filter(s => s.status === 'online').length}/${res.data.length} ${t('common.online')}`); }
    else { setMessage(res.message); }
    setCheckingAll(false); refetch();
  };

  const handleConnectSSH = (server: RemoteServer) => { if (onConnectSSH) onConnectSSH(server.host, server.port, server.user); };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  const servers = data || [];
  const filtered = servers.filter(s => {
    if (!filter) return true;
    const q = filter.toLowerCase();
    return s.name.toLowerCase().includes(q) || s.host.toLowerCase().includes(q) || s.tags.some(t => t.toLowerCase().includes(q)) || s.memo.toLowerCase().includes(q);
  });
  const onlineCount = servers.filter(s => s.status === 'online').length;
  const offlineCount = servers.filter(s => s.status === 'offline').length;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('remote.title')}</h2>
        <div className="server-summary">
          <span className="status-badge up">{onlineCount} {t('common.online')}</span>
          {offlineCount > 0 && <span className="status-badge down">{offlineCount} {t('common.offline')}</span>}
          <span className="text-secondary">{servers.length} {t('remote.total')}</span>
        </div>
        <div className="btn-group">
          <button className="btn btn-primary btn-sm" onClick={() => setShowAdd(!showAdd)}>{showAdd ? t('common.close') : t('remote.addServer')}</button>
          <button className="btn btn-secondary btn-sm" onClick={() => setShowKeyPanel(!showKeyPanel)}>{showKeyPanel ? t('remote.hideKey') : t('remote.sshKey')}</button>
          <button className="btn btn-secondary btn-sm" onClick={handleCheckAll} disabled={checkingAll || servers.length === 0}>{checkingAll ? t('remote.checking') : t('remote.checkAll')}</button>
        </div>
      </div>
      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}
      {showKeyPanel && <SshKeyPanel />}
      {showAdd && <AddServerForm onAdded={() => { refetch(); setShowAdd(false); }} />}
      {servers.length > 3 && <input value={filter} onChange={e => setFilter(e.target.value)} placeholder={t('remote.filterPlaceholder')} className="filter-input" style={{ marginBottom: '8px' }} />}
      {filtered.length === 0 && servers.length === 0 && (
        <div style={{ textAlign: 'center', padding: '40px 0', color: 'var(--text-secondary)' }}>
          <p>{t('remote.noServers')}</p><p>{t('remote.noServersHint')}</p>
          <p style={{ fontSize: '12px', marginTop: '12px' }}>{t('remote.noServersKeyHint')}</p>
        </div>
      )}
      {filtered.map(server => <ServerCard key={server.id} server={server} onRefresh={refetch} onConnectSSH={handleConnectSSH} />)}
    </div>
  );
}

import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../../types';
import { useT } from '../../i18n';

interface Props {
  /** Absent for a new server, present when editing an existing one. */
  server?: RemoteServer;
  onSaved: (server: RemoteServer) => void;
  onCancel?: () => void;
}

/**
 * The one form used both to add a server and to edit it — every field the
 * server record has, so nothing is editable in one place and frozen in the
 * other.
 */
export default function ServerForm({ server, onSaved, onCancel }: Props) {
  const { t } = useT();
  const [name, setName] = useState(server?.name || '');
  const [host, setHost] = useState(server?.host || '');
  const [port, setPort] = useState(String(server?.port || 22));
  const [user, setUser] = useState(server?.user || 'root');
  const [auth, setAuth] = useState(server?.auth_method || 'key');
  const [password, setPassword] = useState('');
  const [tags, setTags] = useState(server?.tags.join(', ') || '');
  const [memo, setMemo] = useState(server?.memo || '');
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);

  /** Prove a freshly saved password authenticates instead of finding out later. */
  const verifyPassword = async (id: string) => {
    setMessage(t('remote.passwordVerifying'));
    const res = await apiPost<RemoteServerStatus>(`/api/remote-servers/${id}/check`, {});
    const ok = res.success && res.data?.status === 'online';
    setMessage(ok ? t('remote.passwordVerified')
      : `${t('remote.passwordVerifyFailed')} ${res.data?.hostname || res.message}`);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const portNum = parseInt(port, 10);
    if (!host.trim()) { setMessage(t('remote.hostRequired')); return; }
    if (!user.trim()) { setMessage(t('remote.userRequired')); return; }
    if (!portNum || portNum < 1 || portNum > 65535) { setMessage(t('remote.portInvalid')); return; }

    setSaving(true);
    const body: Record<string, unknown> = {
      name: name.trim() || host.trim(),
      host: host.trim(), port: portNum, user: user.trim(),
      auth_method: auth, memo: memo.trim(),
      tags: tags.split(',').map(s => s.trim()).filter(Boolean),
    };
    // On edit an empty box keeps the stored password; on add it means none.
    if (password || !server) body.password = auth === 'password' ? password : '';

    const res = server
      ? await apiRequest<RemoteServer>(`/api/remote-servers/${server.id}`, {
          method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
        })
      : await apiPost<RemoteServer>('/api/remote-servers', body);
    setSaving(false);

    if (!res.success || !res.data) { setMessage(res.message || t('common.error')); return; }
    setPassword('');
    onSaved(res.data);
    if (password) verifyPassword(res.data.id);
    else setMessage(server ? t('remote.serverSaved') : t('remote.serverAdded'));
  };

  const clearPassword = async () => {
    if (!server || !window.confirm(t('remote.clearPasswordConfirm'))) return;
    const res = await apiRequest<RemoteServer>(`/api/remote-servers/${server.id}`, {
      method: 'PUT', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ password: '', auth_method: 'key' }),
    });
    if (res.success && res.data) { setAuth('key'); onSaved(res.data); }
  };

  return (
    <form className="rw-form" onSubmit={handleSubmit}>
      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}
      <div className="form-grid">
        <div className="form-group"><label>{t('common.name')}</label>
          <input value={name} onChange={e => setName(e.target.value)} placeholder="web-prod-01" className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.host')} *</label>
          <input value={host} onChange={e => setHost(e.target.value)} placeholder="192.168.1.100" required className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.port')}</label>
          <input value={port} onChange={e => setPort(e.target.value)} type="number" min={1} max={65535} className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.user')} *</label>
          <input value={user} onChange={e => setUser(e.target.value)} placeholder="root" className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.auth')}</label>
          <select value={auth} onChange={e => setAuth(e.target.value)} className="select-input">
            <option value="key">{t('remote.sshKey')}</option>
            <option value="password">{t('accounts.password')}</option>
          </select></div>
        {auth === 'password' && (
          <div className="form-group"><label>{t('remote.password')}</label>
            <input value={password} onChange={e => setPassword(e.target.value)} type="password" autoComplete="new-password"
              className="filter-input"
              placeholder={server?.has_password ? t('remote.passwordKeep') : t('remote.passwordPlaceholder')} />
            {password !== password.trim() && (
              <small style={{ fontSize: 11, color: 'var(--warning)' }}>{t('remote.passwordWhitespace')}</small>
            )}
            {server?.has_password && (
              <button type="button" className="btn btn-secondary btn-sm" style={{ marginTop: 4 }} onClick={clearPassword}>
                {t('remote.clearPassword')}
              </button>
            )}
            <small className="text-secondary" style={{ fontSize: 11 }}>{t('remote.passwordHint')}</small>
          </div>
        )}
        <div className="form-group"><label>{t('remote.tags')}</label>
          <input value={tags} onChange={e => setTags(e.target.value)} placeholder="web, production" className="filter-input" /></div>
        <div className="form-group" style={{ gridColumn: '1 / -1' }}><label>{t('remote.memo')}</label>
          <input value={memo} onChange={e => setMemo(e.target.value)} placeholder="..." className="filter-input" /></div>
      </div>
      <div className="btn-group">
        <button type="submit" className="btn btn-primary btn-sm" disabled={saving}>
          {saving ? t('common.working') : server ? t('common.save') : t('remote.addServer')}
        </button>
        {onCancel && <button type="button" className="btn btn-secondary btn-sm" onClick={onCancel}>{t('common.cancel')}</button>}
      </div>
    </form>
  );
}

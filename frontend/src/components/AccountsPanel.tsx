import React, { useState } from 'react';
import { useApi, apiPost, apiDelete } from '../hooks/useApi';
import { UserAccount } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';
interface Options { shells: string[]; can_write: boolean }
export default function AccountsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<UserAccount[]>('/api/accounts', 10000);
  const options = useApi<Options>('/api/accounts/options');
  const [form, setForm] = useState<'create' | 'password' | 'delete' | null>(null);
  const [username, setUsername] = useState(''); const [password, setPassword] = useState('');
  const [shell, setShell] = useState('/bin/bash'); const [message, setMessage] = useState('');
  const [target, setTarget] = useState<UserAccount | null>(null);
  const [removeHome, setRemoveHome] = useState(false); const [confirmation, setConfirmation] = useState('');
  const [busy, setBusy] = useState(false);
  const canWrite = options.data?.can_write === true && !options.error;
  const current = target && data?.find(row => row.username === target.username);
  const same = Boolean(target && current?.can_manage && current.uid === target.uid && current.home === target.home);
  const close = () => { setForm(null); setPassword(''); setTarget(null); setConfirmation(''); setRemoveHome(false); };
  const open = (kind: 'create' | 'password' | 'delete', row: UserAccount | null = null) => {
    close(); setMessage(''); setUsername(''); setTarget(row); setForm(kind);
  };
  const submit = async (event: React.FormEvent) => {
    event.preventDefault(); if (busy || !canWrite || !form) return;
    if (form !== 'create' && !same) { setMessage(t('accounts.stale')); return; }
    if (form === 'delete' && confirmation !== target?.username) return;
    setBusy(true); setMessage('');
    try {
      const result = form === 'create'
        ? await apiPost<string>('/api/accounts', { username, password, shell })
        : form === 'password'
          ? await apiPost<string>('/api/accounts/password', { username: target!.username, expected_uid: target!.uid, new_password: password })
          : await apiDelete<string>('/api/accounts', { username: target!.username, expected_uid: target!.uid, remove_home: removeHome, confirm_home: removeHome ? target!.home : null });
      setMessage(result.message || result.data || t('accounts.resultUnknown'));
      if (result.success) close();
    } catch { setMessage(t('accounts.resultUnknown')); }
    finally { setPassword(''); setBusy(false); refetch(); }
  };
  const { sorted, toggle, indicator } = useSortable(data || [], 'uid', 'asc');
  const heading = (key: string, label: string) => <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>;
  if (loading) return <div className="panel loading">{t('accounts.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  return <div className="panel">
    <div className="panel-header"><h2>{t('accounts.title')}</h2>{canWrite && <div className="btn-group">
      <button className="btn btn-primary" disabled={busy} onClick={() => open('create')}>{t('accounts.addUser')}</button>
      <button className="btn btn-secondary" disabled={busy} onClick={() => open('password')}>{t('accounts.changePassword')}</button>
    </div>}</div>
    <p>{t('accounts.boundary')}</p><p>{t('accounts.rules')}</p>
    {options.error && <p role="alert">{options.error}</p>}
    {message && <div role="status" className="message">{message}</div>}
    {form && canWrite && <form onSubmit={submit} className="inline-form" style={{ textAlign: 'left' }}>
      <h3>{form === 'create' ? t('accounts.createAccount') : form === 'password' ? t('accounts.changePassword') : t('common.delete')}</h3>
      {form === 'create' ? <>
        <label>{t('accounts.username')}<input aria-label={t('accounts.username')} autoComplete="off" value={username} onChange={e => setUsername(e.target.value)} pattern="[a-z_][a-z0-9_-]{0,31}" maxLength={32} required /></label>
        <label>{t('accounts.shell')}<select aria-label={t('accounts.shell')} value={shell} onChange={e => setShell(e.target.value)} required>{options.data?.shells.map(value => <option key={value}>{value}</option>)}</select></label>
      </> : form === 'password' ? <label>{t('accounts.select')}<select aria-label={t('accounts.select')} value={target?.username || ''} onChange={e => setTarget(data?.find(row => row.username === e.target.value) || null)} required>
        <option value="">—</option>{data?.filter(row => row.can_manage).map(row => <option key={row.username} value={row.username}>{row.username} · UID {row.uid}</option>)}
      </select></label> : null}
      {target && <p><strong>{target.username}</strong> · UID {target.uid} · <code>{target.home}</code></p>}
      {target && !same && <p role="alert">{t('accounts.stale')}</p>}
      {form !== 'delete' ? <label>{t('accounts.newPassword')}<input aria-label={t('accounts.newPassword')} type="password" autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} required minLength={12} maxLength={256} /></label> : <>
        <p>{t('accounts.keepHome')}</p><p>{t('accounts.deleteWarning')}</p>
        <label><input type="checkbox" checked={removeHome} onChange={e => setRemoveHome(e.target.checked)} />{t('accounts.removeHome')}</label>
        <label>{t('accounts.confirmName')}<input aria-label={t('accounts.confirmName')} autoComplete="off" value={confirmation} onChange={e => setConfirmation(e.target.value)} required /></label>
      </>}
      <div className="btn-group"><button type="submit" className={form === 'delete' ? 'btn btn-danger' : 'btn btn-primary'} disabled={busy || (form !== 'create' && !same) || (form === 'delete' && confirmation !== target?.username) || (form === 'create' && !options.data?.shells.includes(shell))}>{form === 'delete' ? t('common.delete') : form === 'create' ? t('accounts.create') : t('common.change')}</button>
        <button type="button" className="btn btn-secondary" disabled={busy} onClick={close}>{t('common.cancel')}</button></div>
    </form>}
    <table className="data-table"><thead><tr>{heading('username', t('accounts.username'))}{heading('uid',t('accounts.uid'))}{heading('gid',t('accounts.gid'))}<th>{t('accounts.home')}</th>{heading('shell',t('accounts.shell'))}<th>{t('common.status')}</th><th>{t('common.actions')}</th></tr></thead>
      <tbody>{sorted.map(row => <tr key={row.username}><td><strong>{row.username}</strong></td><td>{row.uid}</td><td>{row.gid}</td><td><code>{row.home}</code></td><td><code>{row.shell}</code></td>
        <td>{row.is_logged_in === null ? t('accounts.unknownLogin') : row.is_logged_in ? t('common.online') : t('common.offline')}</td>
        <td>{!row.can_manage ? t('accounts.protected') : canWrite && <button className="btn btn-danger btn-sm" disabled={busy} onClick={() => open('delete',row)}>{t('common.delete')}</button>}</td></tr>)}</tbody>
    </table>
  </div>;
}

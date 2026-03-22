import React, { useState } from 'react';
import { useApi, apiPost, apiDelete } from '../hooks/useApi';
import { UserAccount } from '../types';
import { useT } from '../i18n';

export default function AccountsPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<UserAccount[]>('/api/accounts', 10000);
  const [showForm, setShowForm] = useState<'create' | 'password' | null>(null);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [shell, setShell] = useState('/bin/bash');
  const [message, setMessage] = useState('');

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    const res = await apiPost('/api/accounts', { username, password, shell });
    setMessage(res.message);
    if (res.success) {
      setShowForm(null);
      setUsername('');
      setPassword('');
      refetch();
    }
  };

  const handleChangePassword = async (e: React.FormEvent) => {
    e.preventDefault();
    const res = await apiPost('/api/accounts/password', { username, new_password: password });
    setMessage(res.message);
    if (res.success) {
      setShowForm(null);
      setUsername('');
      setPassword('');
    }
  };

  const handleDelete = async (uname: string) => {
    if (!window.confirm(`Delete account "${uname}"?`)) return;
    const res = await apiDelete('/api/accounts', { username: uname });
    setMessage(res.message);
    if (res.success) refetch();
  };

  if (loading) return <div className="panel loading">{t('accounts.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('accounts.title')}</h2>
        <div className="btn-group">
          <button className="btn btn-primary" onClick={() => setShowForm('create')}>
            {t('accounts.addUser')}
          </button>
          <button className="btn btn-secondary" onClick={() => setShowForm('password')}>
            {t('accounts.changePassword')}
          </button>
        </div>
      </div>

      {message && (
        <div className="message" onClick={() => setMessage('')}>{message}</div>
      )}

      {showForm === 'create' && (
        <form onSubmit={handleCreate} className="inline-form">
          <h3>{t('accounts.createAccount')}</h3>
          <input placeholder={t('accounts.username')} value={username}
            onChange={e => setUsername(e.target.value)} required />
          <input placeholder={t('accounts.password')} type="password" value={password}
            onChange={e => setPassword(e.target.value)} required />
          <input placeholder={t('accounts.shell')} value={shell}
            onChange={e => setShell(e.target.value)} />
          <div className="btn-group">
            <button type="submit" className="btn btn-primary">{t('accounts.create')}</button>
            <button type="button" className="btn btn-secondary"
              onClick={() => setShowForm(null)}>{t('common.cancel')}</button>
          </div>
        </form>
      )}

      {showForm === 'password' && (
        <form onSubmit={handleChangePassword} className="inline-form">
          <h3>{t('accounts.changePassword')}</h3>
          <input placeholder={t('accounts.username')} value={username}
            onChange={e => setUsername(e.target.value)} required />
          <input placeholder={t('accounts.newPassword')} type="password" value={password}
            onChange={e => setPassword(e.target.value)} required />
          <div className="btn-group">
            <button type="submit" className="btn btn-primary">{t('common.change')}</button>
            <button type="button" className="btn btn-secondary"
              onClick={() => setShowForm(null)}>{t('common.cancel')}</button>
          </div>
        </form>
      )}

      <table className="data-table">
        <thead>
          <tr>
            <th>{t('accounts.username')}</th>
            <th>{t('accounts.uid')}</th>
            <th>{t('accounts.gid')}</th>
            <th>{t('accounts.home')}</th>
            <th>{t('accounts.shell')}</th>
            <th>{t('common.status')}</th>
            <th>{t('common.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {(data || []).map((acc) => (
            <tr key={acc.username}>
              <td><strong>{acc.username}</strong></td>
              <td>{acc.uid}</td>
              <td>{acc.gid}</td>
              <td><code>{acc.home}</code></td>
              <td><code>{acc.shell}</code></td>
              <td>
                <span className={`status-badge ${acc.is_logged_in ? 'up' : 'down'}`}>
                  {acc.is_logged_in ? t('common.online') : t('common.offline')}
                </span>
              </td>
              <td>
                {acc.username !== 'root' && (
                  <button className="btn btn-danger btn-sm"
                    onClick={() => handleDelete(acc.username)}>{t('common.delete')}</button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

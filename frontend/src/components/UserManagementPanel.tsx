import React, { useState } from 'react';
import { useApi, apiPost, apiRequest } from '../hooks/useApi';
import { useT } from '../i18n';

interface NabimanUser {
  id: string; username: string; role: string;
  created_at: string; last_login: string | null; totp_enabled: boolean;
}

export default function UserManagementPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<NabimanUser[]>('/api/users');
  const [showAdd, setShowAdd] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState('operator');
  const [msg, setMsg] = useState('');

  const addUser = async () => {
    if (!username || !password) return;
    const res = await apiPost<string>('/api/users', { username, password, role });
    setMsg(res.data || res.message);
    if (res.success) { setShowAdd(false); setUsername(''); setPassword(''); refetch(); }
  };

  const deleteUser = async (id: string, name: string) => {
    if (!window.confirm(t('userMgmt.confirmDelete').replace('{name}', name))) return;
    const res = await apiRequest<string>(`/api/users/${id}`, { method: 'DELETE' });
    setMsg(res.data || res.message);
    refetch();
  };

  const changeRole = async (id: string, newRole: string) => {
    const res = await apiRequest<string>(`/api/users/${id}`, {
      method: 'PUT', body: JSON.stringify({ role: newRole }),
    });
    setMsg(res.data || res.message);
    refetch();
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('userMgmt.title')}</h2>
        <button className="btn btn-primary btn-sm" onClick={() => setShowAdd(!showAdd)}>
          {showAdd ? t('common.cancel') : t('userMgmt.addUser')}
        </button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {showAdd && (
        <div className="inline-form">
          <div className="form-row">
            <label>{t('login.username')}</label>
            <input value={username} onChange={e => setUsername(e.target.value)} placeholder="username" />
          </div>
          <div className="form-row">
            <label>{t('login.password')}</label>
            <input type="password" value={password} onChange={e => setPassword(e.target.value)} placeholder="min 8 chars" />
          </div>
          <div className="form-row">
            <label>{t('userMgmt.role')}</label>
            <select className="select-input" value={role} onChange={e => setRole(e.target.value)}>
              <option value="admin">{t('userMgmt.admin')}</option>
              <option value="operator">{t('userMgmt.operator')}</option>
              <option value="viewer">{t('userMgmt.viewer')}</option>
            </select>
          </div>
          <button className="btn btn-primary" onClick={addUser}>{t('accounts.create')}</button>
        </div>
      )}
      <table className="data-table">
        <thead><tr>
          <th>{t('login.username')}</th><th>{t('userMgmt.role')}</th>
          <th>{t('userMgmt.created')}</th><th>{t('userMgmt.lastLogin')}</th>
          <th>{t('common.actions')}</th>
        </tr></thead>
        <tbody>
          {data?.map(u => (
            <tr key={u.id}>
              <td><strong>{u.username}</strong></td>
              <td>
                <select className="select-input" value={u.role} onChange={e => changeRole(u.id, e.target.value)}>
                  <option value="admin">{t('userMgmt.admin')}</option>
                  <option value="operator">{t('userMgmt.operator')}</option>
                  <option value="viewer">{t('userMgmt.viewer')}</option>
                </select>
              </td>
              <td>{u.created_at}</td>
              <td>{u.last_login || '-'}</td>
              <td>
                <button className="btn btn-sm btn-danger" onClick={() => deleteUser(u.id, u.username)}>{t('common.delete')}</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

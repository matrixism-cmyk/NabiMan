import React, { useState } from 'react';
import { useApi, apiPost, apiDelete } from '../hooks/useApi';
import { UserAccount } from '../types';

export default function AccountsPanel() {
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

  if (loading) return <div className="panel loading">Loading accounts...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>User Accounts</h2>
        <div className="btn-group">
          <button className="btn btn-primary" onClick={() => setShowForm('create')}>
            Add User
          </button>
          <button className="btn btn-secondary" onClick={() => setShowForm('password')}>
            Change Password
          </button>
        </div>
      </div>

      {message && (
        <div className="message" onClick={() => setMessage('')}>{message}</div>
      )}

      {showForm === 'create' && (
        <form onSubmit={handleCreate} className="inline-form">
          <h3>Create Account</h3>
          <input placeholder="Username" value={username}
            onChange={e => setUsername(e.target.value)} required />
          <input placeholder="Password" type="password" value={password}
            onChange={e => setPassword(e.target.value)} required />
          <input placeholder="Shell" value={shell}
            onChange={e => setShell(e.target.value)} />
          <div className="btn-group">
            <button type="submit" className="btn btn-primary">Create</button>
            <button type="button" className="btn btn-secondary"
              onClick={() => setShowForm(null)}>Cancel</button>
          </div>
        </form>
      )}

      {showForm === 'password' && (
        <form onSubmit={handleChangePassword} className="inline-form">
          <h3>Change Password</h3>
          <input placeholder="Username" value={username}
            onChange={e => setUsername(e.target.value)} required />
          <input placeholder="New Password" type="password" value={password}
            onChange={e => setPassword(e.target.value)} required />
          <div className="btn-group">
            <button type="submit" className="btn btn-primary">Change</button>
            <button type="button" className="btn btn-secondary"
              onClick={() => setShowForm(null)}>Cancel</button>
          </div>
        </form>
      )}

      <table className="data-table">
        <thead>
          <tr>
            <th>Username</th>
            <th>UID</th>
            <th>GID</th>
            <th>Home</th>
            <th>Shell</th>
            <th>Status</th>
            <th>Actions</th>
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
                  {acc.is_logged_in ? 'Online' : 'Offline'}
                </span>
              </td>
              <td>
                {acc.username !== 'root' && (
                  <button className="btn btn-danger btn-sm"
                    onClick={() => handleDelete(acc.username)}>Delete</button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

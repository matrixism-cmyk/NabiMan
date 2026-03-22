import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { CronJob } from '../types';

export default function CronPanel() {
  const { data, loading, error, refetch } = useApi<CronJob[]>('/api/cron');
  const [showForm, setShowForm] = useState(false);
  const [user, setUser] = useState('root');
  const [schedule, setSchedule] = useState('');
  const [command, setCommand] = useState('');
  const [message, setMessage] = useState('');
  const [actionLoading, setActionLoading] = useState(false);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!schedule.trim() || !command.trim()) return;
    setActionLoading(true);
    setMessage('');
    const res = await apiPost<string>('/api/cron/add', { user, schedule: schedule.trim(), command: command.trim() });
    setMessage(res.success ? res.data || 'Cron job added' : res.message);
    setActionLoading(false);
    if (res.success) {
      setShowForm(false);
      setSchedule('');
      setCommand('');
      refetch();
    }
  };

  const handleDelete = async (cronUser: string, id: number) => {
    if (!window.confirm(`Delete cron job #${id}?`)) return;
    setActionLoading(true);
    const res = await apiPost<string>('/api/cron/delete', { user: cronUser, id });
    setMessage(res.success ? res.data || 'Deleted' : res.message);
    setActionLoading(false);
    refetch();
  };

  if (loading) return <div className="panel loading">Loading cron jobs...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>Scheduled Tasks (Cron)</h2>
        <button className="btn btn-primary" onClick={() => setShowForm(!showForm)}>
          {showForm ? 'Cancel' : 'Add Job'}
        </button>
      </div>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {showForm && (
        <form onSubmit={handleAdd} className="inline-form">
          <h3>New Cron Job</h3>
          <div className="form-row">
            <label>User</label>
            <input value={user} onChange={e => setUser(e.target.value)} placeholder="root" required />
          </div>
          <div className="form-row">
            <label>Schedule</label>
            <input value={schedule} onChange={e => setSchedule(e.target.value)}
              placeholder="*/5 * * * *" required />
            <span className="text-secondary" style={{ fontSize: '12px' }}>
              min hour dom month dow
            </span>
          </div>
          <div className="form-row">
            <label>Command</label>
            <input value={command} onChange={e => setCommand(e.target.value)}
              placeholder="/usr/bin/some-script.sh" required />
          </div>
          <div className="btn-group">
            <button type="submit" className="btn btn-primary" disabled={actionLoading}>Add</button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowForm(false)}>Cancel</button>
          </div>
        </form>
      )}

      <table className="data-table">
        <thead>
          <tr>
            <th>#</th>
            <th>User</th>
            <th>Schedule</th>
            <th>Command</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {(data || []).map(job => (
            <tr key={`${job.user}-${job.id}`}>
              <td>{job.id}</td>
              <td><strong>{job.user}</strong></td>
              <td><code>{job.schedule}</code></td>
              <td><code>{job.command}</code></td>
              <td>
                <button className="btn btn-danger btn-sm" disabled={actionLoading}
                  onClick={() => handleDelete(job.user, job.id)}>Delete</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {(data || []).length === 0 && (
        <p className="text-secondary">No cron jobs found.</p>
      )}
    </div>
  );
}

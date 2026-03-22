import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { FirewallStatus } from '../types';

export default function FirewallPanel() {
  const { data, loading, error, refetch } = useApi<FirewallStatus>('/api/firewall/status', 10000);
  const [port, setPort] = useState('');
  const [protocol, setProtocol] = useState('tcp');
  const [action, setAction] = useState('allow');
  const [message, setMessage] = useState('');
  const [actionLoading, setActionLoading] = useState(false);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!port.trim()) return;
    setActionLoading(true);
    setMessage('');
    const res = await apiPost<string>('/api/firewall/add', { port: port.trim(), protocol, action });
    setMessage(res.success ? res.data || 'Rule added' : res.message);
    setActionLoading(false);
    if (res.success) {
      setPort('');
      refetch();
    }
  };

  const handleDelete = async (num: number) => {
    if (!window.confirm(`Delete rule #${num}?`)) return;
    setActionLoading(true);
    const res = await apiPost<string>('/api/firewall/delete', { number: num });
    setMessage(res.success ? res.data || 'Rule deleted' : res.message);
    setActionLoading(false);
    refetch();
  };

  if (loading) return <div className="panel loading">Loading firewall status...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;
  if (!data) return <div className="panel">No firewall data available</div>;

  return (
    <div className="panel">
      <h2>Firewall Management</h2>

      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">Backend</span>
          <span className="info-value">{data.backend}</span>
        </div>
        <div className="info-item">
          <span className="info-label">Status</span>
          <span className={`status-badge ${data.active ? 'up' : 'down'}`}>
            {data.active ? 'Active' : 'Inactive'}
          </span>
        </div>
      </div>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      <form onSubmit={handleAdd} className="inline-form" style={{ flexDirection: 'row', alignItems: 'center', gap: '8px' }}>
        <input
          placeholder="Port (e.g. 8080)"
          value={port}
          onChange={e => setPort(e.target.value)}
          style={{ width: '120px' }}
        />
        <select value={protocol} onChange={e => setProtocol(e.target.value)} className="select-input">
          <option value="tcp">TCP</option>
          <option value="udp">UDP</option>
          <option value="any">Any</option>
        </select>
        <select value={action} onChange={e => setAction(e.target.value)} className="select-input">
          <option value="allow">Allow</option>
          <option value="deny">Deny</option>
        </select>
        <button type="submit" className="btn btn-primary" disabled={actionLoading || !port.trim()}>
          Add Rule
        </button>
      </form>

      <h3>Rules ({data.rules.length})</h3>
      <table className="data-table">
        <thead>
          <tr>
            <th>#</th>
            <th>Action</th>
            <th>Protocol</th>
            <th>Port</th>
            <th>Source</th>
            <th>Destination</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {data.rules.map(rule => (
            <tr key={rule.number}>
              <td>{rule.number}</td>
              <td>
                <span className={`status-badge ${
                  rule.action.toLowerCase().includes('allow') || rule.action === 'ACCEPT' ? 'up' : 'error'
                }`}>
                  {rule.action}
                </span>
              </td>
              <td>{rule.protocol}</td>
              <td><code>{rule.port}</code></td>
              <td>{rule.source}</td>
              <td>{rule.destination}</td>
              <td>
                <button className="btn btn-danger btn-sm" disabled={actionLoading}
                  onClick={() => handleDelete(rule.number)}>Delete</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {data.rules.length === 0 && (
        <p className="text-secondary">No rules configured.</p>
      )}
    </div>
  );
}

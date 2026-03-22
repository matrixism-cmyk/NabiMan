import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { ServiceConfig, ServiceDefinition } from '../types';

function ConfigEditor({ serviceId, displayName }: { serviceId: string; displayName: string }) {
  const { data, loading, error, refetch } = useApi<ServiceConfig>(`/api/config/${serviceId}`);
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState('');
  const [message, setMessage] = useState('');
  const [actionLoading, setActionLoading] = useState(false);

  const startEdit = () => {
    if (data) {
      setContent(data.content);
      setEditing(true);
    }
  };

  const handleSave = async () => {
    setActionLoading(true);
    const res = await apiPost(`/api/config/${serviceId}`, { content });
    setMessage(res.message);
    if (res.success) {
      setEditing(false);
      refetch();
    }
    setActionLoading(false);
  };

  const handleRestart = async () => {
    if (!window.confirm(`Restart ${displayName}?`)) return;
    setActionLoading(true);
    const res = await apiPost(`/api/config/${serviceId}/restart`, {});
    setMessage(res.message);
    setActionLoading(false);
    setTimeout(refetch, 2000);
  };

  const handleValidate = async () => {
    setActionLoading(true);
    const res = await apiPost<string>(`/api/config/${serviceId}/validate`, {});
    setMessage(res.success ? `Validation: ${res.data || 'OK'}` : res.message);
    setActionLoading(false);
  };

  if (loading) return <div className="loading">Loading {displayName} config...</div>;
  if (error) return <div className="error">Error: {error}</div>;
  if (!data) return null;

  return (
    <div className="config-section">
      <div className="config-header">
        <h3>{displayName}</h3>
        <div className="config-meta">
          <span className={`status-badge ${data.is_running ? 'up' : 'down'}`}>
            {data.is_running ? 'Running' : 'Stopped'}
          </span>
          <code>{data.config_path || 'Not found'}</code>
        </div>
        <div className="btn-group">
          {!editing && <button className="btn btn-primary" onClick={startEdit} disabled={actionLoading}>Edit</button>}
          <button className="btn btn-secondary" onClick={handleValidate} disabled={actionLoading}>Validate</button>
          <button className="btn btn-warning" onClick={handleRestart} disabled={actionLoading}>Restart</button>
        </div>
      </div>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {editing ? (
        <div className="editor-area">
          <textarea
            value={content}
            onChange={e => setContent(e.target.value)}
            className="config-editor"
            spellCheck={false}
          />
          <div className="btn-group">
            <button className="btn btn-primary" onClick={handleSave} disabled={actionLoading}>
              {actionLoading ? 'Saving...' : 'Save'}
            </button>
            <button className="btn btn-secondary" onClick={() => setEditing(false)}>Cancel</button>
          </div>
        </div>
      ) : (
        <pre className="config-preview">{data.content}</pre>
      )}
    </div>
  );
}

export default function ConfigPanel() {
  const { data: services, loading } = useApi<ServiceDefinition[]>('/api/config/services');
  const [filter, setFilter] = useState<'all' | 'found' | 'running'>('found');

  if (loading) return <div className="panel loading">Loading service configurations...</div>;

  const filtered = (services || []).filter(svc => {
    if (filter === 'found') return svc.config_found;
    if (filter === 'running') return svc.is_running;
    return true;
  });

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>Service Configuration</h2>
        <div className="btn-group">
          {(['found', 'running', 'all'] as const).map(f => (
            <button key={f} className={`btn btn-sm ${filter === f ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setFilter(f)}>
              {f === 'found' ? 'Installed' : f === 'running' ? 'Running' : 'All'}
            </button>
          ))}
        </div>
      </div>

      {filtered.length === 0 && (
        <p className="text-secondary">No services matched the filter. Try "All" to see all registered services.</p>
      )}

      {filtered.map(svc => (
        <ConfigEditor key={svc.id} serviceId={svc.id} displayName={svc.display_name} />
      ))}

      <div style={{ marginTop: '12px' }}>
        <p className="text-secondary">
          Registered: {(services || []).map(s => s.display_name).join(', ')}
        </p>
      </div>
    </div>
  );
}

import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { ServiceConfig } from '../types';

function ConfigEditor({ service }: { service: 'apache' | 'tomcat' }) {
  const { data, loading, error, refetch } = useApi<ServiceConfig>(`/api/config/${service}`);
  const [editing, setEditing] = useState(false);
  const [content, setContent] = useState('');
  const [message, setMessage] = useState('');

  const startEdit = () => {
    if (data) {
      setContent(data.content);
      setEditing(true);
    }
  };

  const handleSave = async () => {
    const res = await apiPost(`/api/config/${service}`, {
      service_name: service,
      content,
    });
    setMessage(res.message);
    if (res.success) {
      setEditing(false);
      refetch();
    }
  };

  const handleRestart = async () => {
    if (!window.confirm(`Restart ${service}?`)) return;
    const res = await apiPost(`/api/config/${service}/restart`, {});
    setMessage(res.message);
    setTimeout(refetch, 2000);
  };

  if (loading) return <div className="loading">Loading {service} config...</div>;
  if (error) return <div className="error">Error: {error}</div>;
  if (!data) return null;

  return (
    <div className="config-section">
      <div className="config-header">
        <h3>{service.charAt(0).toUpperCase() + service.slice(1)}</h3>
        <div className="config-meta">
          <span className={`status-badge ${data.is_running ? 'up' : 'down'}`}>
            {data.is_running ? 'Running' : 'Stopped'}
          </span>
          <code>{data.config_path || 'Not found'}</code>
        </div>
        <div className="btn-group">
          {!editing && <button className="btn btn-primary" onClick={startEdit}>Edit</button>}
          <button className="btn btn-secondary" onClick={handleRestart}>Restart</button>
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
            <button className="btn btn-primary" onClick={handleSave}>Save</button>
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
  return (
    <div className="panel">
      <h2>Service Configuration</h2>
      <ConfigEditor service="apache" />
      <ConfigEditor service="tomcat" />
    </div>
  );
}

import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { Container, ContainerImage } from '../types';

export default function ContainersPanel() {
  const { data: containers, loading, error, refetch } = useApi<Container[]>('/api/containers', 5000);
  const { data: images, refetch: refetchImages } = useApi<ContainerImage[]>('/api/containers/images');
  const [message, setMessage] = useState('');
  const [logContent, setLogContent] = useState('');
  const [logContainer, setLogContainer] = useState('');
  const [actionLoading, setActionLoading] = useState(false);
  const [showImages, setShowImages] = useState(false);

  const action = async (endpoint: string, id: string, label: string) => {
    if (label === 'Remove' && !window.confirm(`Remove container "${id}"?`)) return;
    setActionLoading(true);
    setMessage('');
    const res = await apiPost<string>(`/api/containers/${endpoint}`, { id });
    setMessage(res.success ? res.data || 'OK' : res.message);
    setActionLoading(false);
    refetch();
  };

  const viewLogs = async (id: string, name: string) => {
    const res = await apiPost<string>('/api/containers/logs', { id });
    if (res.success && res.data) {
      setLogContent(res.data);
      setLogContainer(name);
    } else {
      setMessage(res.message);
    }
  };

  const removeImage = async (id: string) => {
    if (!window.confirm(`Remove image "${id}"?`)) return;
    setActionLoading(true);
    const res = await apiPost<string>('/api/containers/images/remove', { id });
    setMessage(res.success ? res.data || 'OK' : res.message);
    setActionLoading(false);
    refetchImages();
  };

  if (loading) return <div className="panel loading">Loading containers...</div>;
  if (error) return <div className="panel error">Error: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>Container Management</h2>
        <div className="btn-group">
          <button className="btn btn-secondary" onClick={() => setShowImages(!showImages)}>
            {showImages ? 'Containers' : 'Images'}
          </button>
        </div>
      </div>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {logContainer && (
        <div className="log-viewer">
          <div className="log-header">
            <h3>Logs: {logContainer}</h3>
            <button className="btn btn-secondary btn-sm" onClick={() => setLogContainer('')}>Close</button>
          </div>
          <pre className="log-content">{logContent}</pre>
        </div>
      )}

      {!showImages ? (
        <>
          <h3>Containers ({(containers || []).length})</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Image</th>
                <th>Status</th>
                <th>Ports</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {(containers || []).map((c) => (
                <tr key={c.id}>
                  <td><strong>{c.name}</strong></td>
                  <td>{c.image}</td>
                  <td>
                    <span className={`status-badge ${c.state === 'running' ? 'up' : 'down'}`}>
                      {c.state}
                    </span>
                  </td>
                  <td><code>{c.ports || '-'}</code></td>
                  <td>
                    <div className="btn-group">
                      {c.state !== 'running' && (
                        <button className="btn btn-primary btn-sm" disabled={actionLoading}
                          onClick={() => action('start', c.id, 'Start')}>Start</button>
                      )}
                      {c.state === 'running' && (
                        <button className="btn btn-warning btn-sm" disabled={actionLoading}
                          onClick={() => action('stop', c.id, 'Stop')}>Stop</button>
                      )}
                      <button className="btn btn-secondary btn-sm" disabled={actionLoading}
                        onClick={() => action('restart', c.id, 'Restart')}>Restart</button>
                      <button className="btn btn-secondary btn-sm"
                        onClick={() => viewLogs(c.id, c.name)}>Logs</button>
                      <button className="btn btn-danger btn-sm" disabled={actionLoading}
                        onClick={() => action('remove', c.id, 'Remove')}>Remove</button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {(containers || []).length === 0 && (
            <p className="text-secondary">No containers found. Docker may not be installed or running.</p>
          )}
        </>
      ) : (
        <>
          <h3>Images ({(images || []).length})</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>Repository</th>
                <th>Tag</th>
                <th>ID</th>
                <th>Size</th>
                <th>Created</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {(images || []).map((img) => (
                <tr key={img.id}>
                  <td><strong>{img.repository}</strong></td>
                  <td>{img.tag}</td>
                  <td><code>{img.id.substring(0, 12)}</code></td>
                  <td>{img.size}</td>
                  <td>{img.created}</td>
                  <td>
                    <button className="btn btn-danger btn-sm" disabled={actionLoading}
                      onClick={() => removeImage(img.id)}>Remove</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}

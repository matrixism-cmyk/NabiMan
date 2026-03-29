import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { Container, ContainerImage } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

export default function ContainersPanel() {
  const { t } = useT();
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

  const containerList = containers || [];
  const { sorted, toggle, indicator } = useSortable(containerList, 'name', 'asc');
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  if (loading) return <div className="panel loading">{t('containers.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('containers.title')}</h2>
        <div className="btn-group">
          <button className="btn btn-secondary" onClick={() => setShowImages(!showImages)}>
            {showImages ? t('containers.containers') : t('containers.images')}
          </button>
        </div>
      </div>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {logContainer && (
        <div className="log-viewer">
          <div className="log-header">
            <h3>{t('containers.logs')}: {logContainer}</h3>
            <button className="btn btn-secondary btn-sm" onClick={() => setLogContainer('')}>{t('common.close')}</button>
          </div>
          <pre className="log-content">{logContent}</pre>
        </div>
      )}

      {!showImages ? (
        <>
          <h3>{t('containers.containers')} ({containerList.length})</h3>
          <table className="data-table">
            <thead>
              <tr>
                {S('name', t('common.name'))}
                {S('image', t('containers.image'))}
                {S('state', t('common.status'))}
                <th>{t('containers.ports')}</th>
                <th>{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody>
              {sorted.map((c) => (
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
                          onClick={() => action('start', c.id, 'Start')}>{t('common.start')}</button>
                      )}
                      {c.state === 'running' && (
                        <button className="btn btn-warning btn-sm" disabled={actionLoading}
                          onClick={() => action('stop', c.id, 'Stop')}>{t('common.stop')}</button>
                      )}
                      <button className="btn btn-secondary btn-sm" disabled={actionLoading}
                        onClick={() => action('restart', c.id, 'Restart')}>{t('common.restart')}</button>
                      <button className="btn btn-secondary btn-sm"
                        onClick={() => viewLogs(c.id, c.name)}>{t('containers.logs')}</button>
                      <button className="btn btn-danger btn-sm" disabled={actionLoading}
                        onClick={() => action('remove', c.id, 'Remove')}>{t('common.remove')}</button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {sorted.length === 0 && (
            <p className="text-secondary">{t('containers.noContainers')}</p>
          )}
        </>
      ) : (
        <>
          <h3>{t('containers.images')} ({(images || []).length})</h3>
          <table className="data-table">
            <thead>
              <tr>
                <th>{t('containers.repository')}</th>
                <th>{t('containers.tag')}</th>
                <th>{t('containers.id')}</th>
                <th>{t('containers.size')}</th>
                <th>{t('containers.created')}</th>
                <th>{t('common.actions')}</th>
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
                      onClick={() => removeImage(img.id)}>{t('common.remove')}</button>
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

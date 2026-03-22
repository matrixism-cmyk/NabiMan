import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { ServiceConfig, ServiceDefinition } from '../types';
import { useT } from '../i18n';

function ConfigEditor({ serviceId, displayName }: { serviceId: string; displayName: string }) {
  const { t } = useT();
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
    setMessage(res.success ? `${t('config.validate')}: ${res.data || 'OK'}` : res.message);
    setActionLoading(false);
  };

  if (loading) return <div className="loading">{t('common.loading')}</div>;
  if (error) return <div className="error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  return (
    <div className="config-section">
      <div className="config-header">
        <h3>{displayName}</h3>
        <div className="config-meta">
          <span className={`status-badge ${data.is_running ? 'up' : 'down'}`}>
            {data.is_running ? t('config.running') : t('config.stopped')}
          </span>
          <code>{data.config_path || 'Not found'}</code>
        </div>
        <div className="btn-group">
          {!editing && <button className="btn btn-primary" onClick={startEdit} disabled={actionLoading}>{t('common.edit')}</button>}
          <button className="btn btn-secondary" onClick={handleValidate} disabled={actionLoading}>{t('config.validate')}</button>
          <button className="btn btn-warning" onClick={handleRestart} disabled={actionLoading}>{t('common.restart')}</button>
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
              {actionLoading ? t('config.saving') : t('common.save')}
            </button>
            <button className="btn btn-secondary" onClick={() => setEditing(false)}>{t('common.cancel')}</button>
          </div>
        </div>
      ) : (
        <pre className="config-preview">{data.content}</pre>
      )}
    </div>
  );
}

function ServiceCard({ svc }: { svc: ServiceDefinition }) {
  const { t } = useT();
  const [expanded, setExpanded] = useState(false);

  return (
    <div key={svc.id}>
      <div className="service-detect-header" onClick={() => setExpanded(!expanded)}
        style={{ cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 0' }}>
        <span style={{ fontSize: '10px', color: 'var(--text-secondary)' }}>{expanded ? '▼' : '▶'}</span>
        <strong>{svc.display_name}</strong>
        <span className={`status-badge ${svc.is_running ? 'up' : 'down'}`} style={{ fontSize: '11px' }}>
          {svc.is_running ? t('config.running') : t('config.stopped')}
        </span>
        {svc.version && (
          <code style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{svc.version}</code>
        )}
      </div>
      {expanded && <ConfigEditor serviceId={svc.id} displayName={svc.display_name} />}
    </div>
  );
}

type FilterMode = 'installed' | 'running' | 'all';

export default function ConfigPanel() {
  const { t } = useT();
  const { data: services, loading } = useApi<ServiceDefinition[]>('/api/config/services');
  const [filter, setFilter] = useState<FilterMode>('installed');

  if (loading) return <div className="panel loading">{t('config.loading')}</div>;

  const allServices = services || [];
  const installedCount = allServices.filter(s => s.is_installed).length;
  const runningCount = allServices.filter(s => s.is_running).length;

  const filtered = allServices.filter(svc => {
    if (filter === 'installed') return svc.is_installed;
    if (filter === 'running') return svc.is_running;
    return true;
  });

  const filterButtons: { key: FilterMode; labelKey: string; count: number }[] = [
    { key: 'installed', labelKey: 'config.installed', count: installedCount },
    { key: 'running', labelKey: 'config.running', count: runningCount },
    { key: 'all', labelKey: 'common.all', count: allServices.length },
  ];

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('config.title')}</h2>
        <div className="btn-group">
          {filterButtons.map(f => (
            <button key={f.key}
              className={`btn btn-sm ${filter === f.key ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setFilter(f.key)}>
              {t(f.labelKey)} ({f.count})
            </button>
          ))}
        </div>
      </div>

      {filtered.length === 0 ? (
        <p className="text-secondary">{t('config.noServices')}</p>
      ) : (
        <p className="text-secondary" style={{ marginBottom: '8px' }}>
          {filtered.length} {t('services.services')}
        </p>
      )}

      {filtered.map(svc => (
        <div key={svc.id} style={{
          opacity: filter === 'all' && !svc.is_installed ? 0.5 : 1,
          borderBottom: '1px solid var(--border)',
          paddingBottom: '4px',
          marginBottom: '4px',
        }}>
          <ServiceCard svc={svc} />
        </div>
      ))}
    </div>
  );
}

import React, { useState, useEffect, useCallback } from 'react';
import { useApi, apiPost, apiRequest } from '../hooks/useApi';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

interface BackupEntry { id: string; source: string; filename: string; size: number; created: string; }
interface BackupSchedule { id: string; name: string; paths: string[]; cron_expr: string; compression: boolean; enabled: boolean; last_run: string | null; next_run: string | null; }

export default function BackupPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<BackupEntry[]>('/api/backup');
  const [path, setPath] = useState('');
  const [msg, setMsg] = useState('');
  const [tab, setTab] = useState<'backups' | 'schedules'>('backups');

  // Schedule state
  const [schedules, setSchedules] = useState<BackupSchedule[]>([]);
  const [schedulesLoading, setSchedulesLoading] = useState(false);
  const [showAddSchedule, setShowAddSchedule] = useState(false);
  const [schedName, setSchedName] = useState('');
  const [schedPaths, setSchedPaths] = useState('');
  const [schedCron, setSchedCron] = useState('0 2 * * *');
  const [schedCompression, setSchedCompression] = useState(true);

  // Compressed backup state
  const [compressedPaths, setCompressedPaths] = useState('');

  const fetchSchedules = useCallback(async () => {
    setSchedulesLoading(true);
    try {
      const token = sessionStorage.getItem('nabiman_token') || '';
      const res = await fetch('/api/backup/schedules', {
        headers: { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' },
      });
      const json = await res.json();
      if (json.success && json.data) setSchedules(json.data);
    } catch { /* ignore */ }
    setSchedulesLoading(false);
  }, []);

  useEffect(() => {
    if (tab === 'schedules') fetchSchedules();
  }, [tab, fetchSchedules]);

  const createBackup = async () => {
    if (!path.trim()) return;
    const res = await apiPost<string>('/api/backup/create', { path });
    setMsg(res.data || res.message);
    refetch();
  };

  const restoreBackup = async (filename: string, source: string) => {
    const target = window.prompt(t('backup.restoreTo'), source);
    if (!target) return;
    const res = await apiPost<string>('/api/backup/restore', { filename, target });
    setMsg(res.data || res.message);
  };

  const deleteBackup = async (filename: string) => {
    if (!window.confirm(t('backup.confirmDelete'))) return;
    const res = await apiPost<string>('/api/backup/delete', { filename });
    setMsg(res.data || res.message);
    refetch();
  };

  const addSchedule = async () => {
    if (!schedName.trim() || !schedPaths.trim() || !schedCron.trim()) return;
    const paths = schedPaths.split(',').map(p => p.trim()).filter(Boolean);
    const res = await apiPost<string>('/api/backup/schedules', {
      name: schedName, paths, cron_expr: schedCron, compression: schedCompression,
    });
    setMsg(res.data || res.message);
    if (res.success) {
      setShowAddSchedule(false);
      setSchedName('');
      setSchedPaths('');
      setSchedCron('0 2 * * *');
      setSchedCompression(true);
      fetchSchedules();
    }
  };

  const deleteSchedule = async (id: string) => {
    if (!window.confirm(t('backup.confirmDelete'))) return;
    await apiRequest<string>(`/api/backup/schedules/${id}`, { method: 'DELETE' });
    fetchSchedules();
  };

  const createCompressed = async () => {
    if (!compressedPaths.trim()) return;
    const paths = compressedPaths.split(',').map(p => p.trim()).filter(Boolean);
    const res = await apiPost<string>('/api/backup/create-compressed', { paths });
    setMsg(res.data || res.message);
    setCompressedPaths('');
    refetch();
  };

  const { sorted, toggle, indicator } = useSortable(data || [], 'created', 'desc');
  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <h2>{t('backup.title')}</h2>
      <div className="tab-row" style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <button
          className={`btn btn-sm ${tab === 'backups' ? 'btn-primary' : 'btn-secondary'}`}
          onClick={() => setTab('backups')}
        >
          {t('backup.title')}
        </button>
        <button
          className={`btn btn-sm ${tab === 'schedules' ? 'btn-primary' : 'btn-secondary'}`}
          onClick={() => setTab('schedules')}
        >
          {t('backup.schedules')}
        </button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}

      {tab === 'backups' && (
        <>
          <div className="filter-row">
            <input className="filter-input" placeholder={t('backup.pathPlaceholder')} value={path}
              onChange={e => setPath(e.target.value)} onKeyDown={e => e.key === 'Enter' && createBackup()} />
            <button className="btn btn-primary btn-sm" onClick={createBackup}>{t('backup.create')}</button>
          </div>
          {data && data.length > 0 ? (
            <table className="data-table">
              <thead><tr>{S('source', t('backup.source'))}{S('filename', t('backup.filename'))}{S('size', t('files.size'))}{S('created', t('backup.created'))}<th>{t('common.actions')}</th></tr></thead>
              <tbody>
                {sorted.map((b, i) => (
                  <tr key={i}>
                    <td><code>{b.source}</code></td>
                    <td>{b.filename}</td>
                    <td>{b.size}</td>
                    <td>{b.created}</td>
                    <td className="btn-group">
                      <button className="btn btn-sm btn-primary" onClick={() => restoreBackup(b.filename, b.source)}>{t('backup.restore')}</button>
                      <button className="btn btn-sm btn-danger" onClick={() => deleteBackup(b.filename)}>{t('common.delete')}</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : <p className="text-secondary">{t('backup.noBackups')}</p>}
        </>
      )}

      {tab === 'schedules' && (
        <>
          <div className="filter-row" style={{ marginBottom: 12 }}>
            <button className="btn btn-primary btn-sm" onClick={() => setShowAddSchedule(!showAddSchedule)}>
              {showAddSchedule ? t('common.cancel') : t('backup.addSchedule')}
            </button>
          </div>

          {showAddSchedule && (
            <div className="inline-form" style={{ marginBottom: 16 }}>
              <div className="form-row">
                <label>{t('backup.scheduleName')}</label>
                <input value={schedName} onChange={e => setSchedName(e.target.value)} placeholder="Daily /var backup" />
              </div>
              <div className="form-row">
                <label>{t('backup.paths')}</label>
                <input value={schedPaths} onChange={e => setSchedPaths(e.target.value)} placeholder="/var/www, /etc/nginx" />
              </div>
              <div className="form-row">
                <label>{t('backup.cronExpr')}</label>
                <input value={schedCron} onChange={e => setSchedCron(e.target.value)} placeholder="0 2 * * *" />
              </div>
              <div className="form-row">
                <label>{t('backup.compression')}</label>
                <input type="checkbox" checked={schedCompression} onChange={e => setSchedCompression(e.target.checked)} />
              </div>
              <button className="btn btn-primary" onClick={addSchedule}>{t('common.add')}</button>
            </div>
          )}

          {/* Compressed Backup */}
          <div className="filter-row" style={{ marginBottom: 12 }}>
            <input className="filter-input" placeholder={t('backup.paths')} value={compressedPaths}
              onChange={e => setCompressedPaths(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && createCompressed()} />
            <button className="btn btn-primary btn-sm" onClick={createCompressed}>{t('backup.createCompressed')}</button>
          </div>

          {schedulesLoading ? (
            <p className="loading">{t('common.loading')}</p>
          ) : schedules.length > 0 ? (
            <table className="data-table">
              <thead>
                <tr>
                  <th>{t('common.name')}</th>
                  <th>{t('backup.paths')}</th>
                  <th>{t('backup.cronExpr')}</th>
                  <th>{t('backup.compression')}</th>
                  <th>{t('common.status')}</th>
                  <th>{t('common.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {schedules.map(s => (
                  <tr key={s.id}>
                    <td><strong>{s.name}</strong></td>
                    <td><code>{s.paths.join(', ')}</code></td>
                    <td><code>{s.cron_expr}</code></td>
                    <td>{s.compression ? t('backup.compressed') : '-'}</td>
                    <td>
                      {s.enabled ? '✓' : '✗'}
                      {s.last_run && <span style={{ marginLeft: 8, fontSize: '0.85em' }}>{s.last_run}</span>}
                    </td>
                    <td>
                      <button className="btn btn-sm btn-danger" onClick={() => deleteSchedule(s.id)}>
                        {t('common.delete')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="text-secondary">{t('backup.noSchedules')}</p>
          )}
        </>
      )}
    </div>
  );
}

import React, { useState, useEffect, useCallback } from 'react';
import { useApi, apiPost, apiRequest, fetchWithRefresh } from '../hooks/useApi';
import { useT } from '../i18n';
import EncryptedBackupPanel from './EncryptedBackupPanel';
import { useSortable } from '../hooks/useSortable';

interface BackupEntry { id: string; source: string; filename: string; size: number; created: string; }
interface BackupSchedule { id: string; name: string; paths: string[]; cron_expr: string; compression: boolean; enabled: boolean; last_run: string | null; next_run: string | null; last_result?: { state: string; started_at: string; finished_at: string | null; filename: string | null; error: string | null } | null; }

export default function BackupPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<BackupEntry[]>('/api/backup');
  const [path, setPath] = useState('');
  const [msg, setMsg] = useState('');
  const [tab, setTab] = useState<'backups' | 'schedules'>('backups');

  // Schedule state
  const [schedules, setSchedules] = useState<BackupSchedule[]>([]);
  const [schedulesLoading, setSchedulesLoading] = useState(false);
  const [scheduleError, setScheduleError] = useState('');
  const [acting, setActing] = useState(false);
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
      const res = await fetchWithRefresh('/api/backup/schedules');
      const json = await res.json();
      if (!res.ok || !json.success) throw new Error(json.message || 'Backup schedules unavailable');
      setSchedules(json.data); setScheduleError('');
    } catch (cause) { setScheduleError(cause instanceof Error ? cause.message : 'Backup schedules unavailable'); }
    setSchedulesLoading(false);
  }, []);

  useEffect(() => {
    if (tab !== 'schedules') return;
    fetchSchedules();
    const timer = window.setInterval(fetchSchedules, 5000);
    return () => window.clearInterval(timer);
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
    setActing(true);
    try {
      const res = await apiPost<BackupSchedule>('/api/backup/schedules', {
        name: schedName, paths, cron_expr: schedCron, compression: schedCompression,
      });
      if (!res.success) throw new Error(res.message);
      setMsg(t('backup.scheduleSaved')); setShowAddSchedule(false);
      setSchedName(''); setSchedPaths(''); setSchedCron('0 2 * * *'); setSchedCompression(true);
      await fetchSchedules();
    } catch (cause) { setMsg(cause instanceof Error ? cause.message : t('common.error')); }
    finally { setActing(false); }
  };

  const scheduleAction = async (id: string, action: 'delete' | 'run') => {
    if (action === 'delete' && !window.confirm(t('backup.confirmDelete'))) return;
    setActing(true);
    try {
      const res = await apiRequest(`/api/backup/schedules/${id}${action === 'run' ? '/run' : ''}`, { method: action === 'run' ? 'POST' : 'DELETE' });
      if (!res.success) throw new Error(res.message);
      setMsg(t(action === 'run' ? 'backup.accepted' : 'backup.scheduleDeleted'));
      await fetchSchedules();
    } catch (cause) { setMsg(cause instanceof Error ? cause.message : t('common.error')); }
    finally { setActing(false); }
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

  if (loading && !data) return <div className="panel loading">{t('common.loading')}</div>;
  if (error && !data) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel backup-panel">
      <h2>{t('backup.title')}</h2>
      {error && <p role="alert">{error}</p>}
      <p>{t('backup.scopeNotice')}</p>
      <details><summary>{t('recovery.title')}</summary><p>{t('recovery.description')}</p><a href="/management-recovery.html" target="_blank" rel="noopener noreferrer">{t('recovery.guide')}</a></details>
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
      {msg && <div role="status" className="message" onClick={() => setMsg('')}>{msg}</div>}

      {tab === 'backups' && (
        <>
          <EncryptedBackupPanel files={(data || []).map(file => file.filename)} refresh={refetch} />
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
                    <td><code>{b.filename.endsWith('.bak') ? b.source : '—'}</code></td>
                    <td>{b.filename}</td>
                    <td>{b.size}</td>
                    <td>{b.created}</td>
                    <td className="btn-group">
                      <button className="btn btn-sm btn-primary" disabled={!b.filename.endsWith('.bak')} title={t('backup.archiveRestore')} onClick={() => restoreBackup(b.filename, b.source)}>{t('backup.restore')}</button>
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
          <p>{t('backup.cronHelp')}</p>
          {scheduleError && <p role="alert">{scheduleError}</p>}
          <div className="filter-row" style={{ marginBottom: 12 }}>
            <button className="btn btn-primary btn-sm" onClick={() => setShowAddSchedule(!showAddSchedule)}>
              {showAddSchedule ? t('common.cancel') : t('backup.addSchedule')}
            </button>
          </div>

          {showAddSchedule && (
            <div className="inline-form" style={{ marginBottom: 16 }}>
              <div className="form-row">
                <label>{t('backup.scheduleName')}</label>
                <input aria-label={t('backup.scheduleName')} value={schedName} onChange={e => setSchedName(e.target.value)} placeholder="Daily /var backup" />
              </div>
              <div className="form-row">
                <label>{t('backup.paths')}</label>
                <input aria-label={t('backup.paths')} value={schedPaths} onChange={e => setSchedPaths(e.target.value)} placeholder="/var/www, /etc/nginx" />
              </div>
              <div className="form-row">
                <label>{t('backup.cronExpr')}</label>
                <input aria-label={t('backup.cronExpr')} value={schedCron} onChange={e => setSchedCron(e.target.value)} placeholder="0 2 * * *" />
              </div>
              <div className="form-row">
                <label>{t('backup.compression')}</label>
                <input type="checkbox" checked={schedCompression} onChange={e => setSchedCompression(e.target.checked)} />
              </div>
              <button className="btn btn-primary" disabled={acting || Boolean(scheduleError)} onClick={addSchedule}>{t('common.add')}</button>
            </div>
          )}

          {/* Compressed Backup */}
          <div className="filter-row" style={{ marginBottom: 12 }}>
            <input className="filter-input" placeholder={t('backup.paths')} value={compressedPaths}
              onChange={e => setCompressedPaths(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && createCompressed()} />
            <button className="btn btn-primary btn-sm" onClick={createCompressed}>{t('backup.createCompressed')}</button>
          </div>

          {schedulesLoading && !schedules.length ? (
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
                      <div>{t('backup.' + (s.last_result?.state || 'unverified'))}</div>
                      {s.last_result && <><div>{s.last_result.started_at}</div><div>{s.last_result.finished_at}</div><code>{s.last_result.filename}</code><p>{s.last_result.error}</p></>}
                      {s.last_run && <div>{t('backup.lastRecorded')}: {s.last_run}</div>}
                    </td>
                    <td>
                      <button className="btn btn-sm btn-primary" disabled={acting || Boolean(scheduleError) || schedules.some(row => row.last_result?.state === 'running')} onClick={() => scheduleAction(s.id, 'run')}>{t('backup.runNow')}</button>
                      <button className="btn btn-sm btn-danger" disabled={acting || Boolean(scheduleError) || s.last_result?.state === 'running'} onClick={() => scheduleAction(s.id, 'delete')}>
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

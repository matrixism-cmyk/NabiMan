import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface BackupEntry { id: string; source: string; filename: string; size: number; created: string; }

export default function BackupPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<BackupEntry[]>('/api/backup');
  const [path, setPath] = useState('');
  const [msg, setMsg] = useState('');

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

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  return (
    <div className="panel">
      <h2>{t('backup.title')}</h2>
      <div className="filter-row">
        <input className="filter-input" placeholder={t('backup.pathPlaceholder')} value={path}
          onChange={e => setPath(e.target.value)} onKeyDown={e => e.key === 'Enter' && createBackup()} />
        <button className="btn btn-primary btn-sm" onClick={createBackup}>{t('backup.create')}</button>
      </div>
      {msg && <div className="message" onClick={() => setMsg('')}>{msg}</div>}
      {data && data.length > 0 ? (
        <table className="data-table">
          <thead><tr><th>{t('backup.source')}</th><th>{t('backup.filename')}</th><th>{t('backup.created')}</th><th>{t('common.actions')}</th></tr></thead>
          <tbody>
            {data.map((b, i) => (
              <tr key={i}>
                <td><code>{b.source}</code></td>
                <td>{b.filename}</td>
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
    </div>
  );
}

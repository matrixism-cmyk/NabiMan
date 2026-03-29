import React, { useState, useEffect } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

interface DbStatus { engine: string; running: boolean; version: string; uptime: string; connections: string; databases: string[]; slow_queries: string; }
interface DbBackupEntry { filename: string; size: number; created: string; }
interface DbQueryResult { columns: string[]; rows: string[][]; row_count: number; }

function fmtSize(b: number): string {
  if (b === 0) return '-';
  if (b < 1024) return b + ' B';
  if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
  if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
  return (b / 1073741824).toFixed(1) + ' GB';
}

export default function DatabasePanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<DbStatus>('/api/database/status', 10000);

  const [backupDb, setBackupDb] = useState('');
  const [backups, setBackups] = useState<DbBackupEntry[]>([]);
  const [backupMsg, setBackupMsg] = useState('');

  const [queryDb, setQueryDb] = useState('');
  const [sql, setSql] = useState('');
  const [queryResult, setQueryResult] = useState<DbQueryResult | null>(null);
  const [queryMsg, setQueryMsg] = useState('');

  const fetchBackups = async () => {
    try {
      const token = sessionStorage.getItem('nabiman_token') || '';
      const res = await fetch('/api/database/backups', {
        headers: { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' },
      });
      const json = await res.json();
      if (json.success && json.data) setBackups(json.data);
    } catch { /* ignore */ }
  };

  useEffect(() => { fetchBackups(); }, []);

  useEffect(() => {
    if (data?.databases?.length && !backupDb) {
      setBackupDb(data.databases[0]);
      setQueryDb(data.databases[0]);
    }
  }, [data, backupDb]);

  const createBackup = async () => {
    if (!backupDb) return;
    const res = await apiPost<string>('/api/database/backup', { database: backupDb });
    setBackupMsg(res.data || res.message);
    fetchBackups();
  };

  const restoreBackup = async (filename: string) => {
    if (!backupDb) return;
    if (!window.confirm(t('db.confirmRestore'))) return;
    const res = await apiPost<string>('/api/database/restore', { database: backupDb, filename });
    setBackupMsg(res.data || res.message);
  };

  const deleteBackup = async (filename: string) => {
    if (!window.confirm(t('backup.confirmDelete'))) return;
    const res = await apiPost<string>('/api/backup/delete', { filename });
    setBackupMsg(res.data || res.message);
    fetchBackups();
  };

  const executeQuery = async () => {
    if (!queryDb || !sql.trim()) return;
    setQueryMsg('');
    setQueryResult(null);
    const res = await apiPost<DbQueryResult>('/api/database/query', { database: queryDb, query: sql });
    if (res.success && res.data) {
      setQueryResult(res.data);
    } else {
      setQueryMsg(res.message);
    }
  };

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const dbOptions = data.databases || [];

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('db.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
      </div>
      <div className="info-grid">
        <div className="info-item"><span className="info-label">{t('db.engine')}</span><span className="info-value">{data.engine}</span></div>
        <div className="info-item"><span className="info-label">{t('db.version')}</span><span className="info-value">{data.version}</span></div>
        <div className="info-item"><span className="info-label">{t('serverStatus.uptime')}</span><span className="info-value">{data.uptime}</span></div>
        <div className="info-item"><span className="info-label">{t('db.connections')}</span><span className="info-value">{data.connections}</span></div>
        <div className="info-item"><span className="info-label">{t('db.slowQueries')}</span><span className="info-value">{data.slow_queries}</span></div>
      </div>
      <h3>{t('db.databases')}</h3>
      <div className="info-grid">
        {data.databases.map((db, i) => (
          <div key={i} className="info-item"><span className="info-value">{db}</span></div>
        ))}
      </div>

      {/* DB Backup Section */}
      <h3>{t('db.backup')}</h3>
      <div className="filter-row">
        <select className="select-input" value={backupDb} onChange={e => setBackupDb(e.target.value)}>
          <option value="">{t('db.selectDb')}</option>
          {dbOptions.map((db, i) => <option key={i} value={db}>{db}</option>)}
        </select>
        <button className="btn btn-primary btn-sm" onClick={createBackup}>{t('db.backup')}</button>
      </div>
      {backupMsg && <div className="message" onClick={() => setBackupMsg('')}>{backupMsg}</div>}
      <h4>{t('db.backups')}</h4>
      {backups.length > 0 ? (
        <table className="data-table">
          <thead>
            <tr>
              <th>{t('backup.filename')}</th>
              <th>{t('files.size')}</th>
              <th>{t('backup.created')}</th>
              <th>{t('common.actions')}</th>
            </tr>
          </thead>
          <tbody>
            {backups.map((b, i) => (
              <tr key={i}>
                <td><code>{b.filename}</code></td>
                <td>{fmtSize(b.size)}</td>
                <td>{b.created}</td>
                <td className="btn-group">
                  <button className="btn btn-sm btn-primary" onClick={() => restoreBackup(b.filename)}>
                    {t('db.restore')}
                  </button>
                  <button className="btn btn-sm btn-danger" onClick={() => deleteBackup(b.filename)}>
                    {t('common.delete')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="text-secondary">{t('db.noBackups')}</p>
      )}

      {/* Query Runner Section */}
      <h3>{t('db.query')}</h3>
      <p className="text-secondary" style={{ fontSize: '0.85em', marginBottom: 8 }}>{t('db.readOnlyHint')}</p>
      <div className="filter-row">
        <select className="select-input" value={queryDb} onChange={e => setQueryDb(e.target.value)}>
          <option value="">{t('db.selectDb')}</option>
          {dbOptions.map((db, i) => <option key={i} value={db}>{db}</option>)}
        </select>
      </div>
      <textarea
        className="file-editor"
        style={{ minHeight: 80, marginBottom: 8 }}
        placeholder="SELECT * FROM ..."
        value={sql}
        onChange={e => setSql(e.target.value)}
      />
      <div className="filter-row">
        <button className="btn btn-primary btn-sm" onClick={executeQuery}>{t('db.execute')}</button>
      </div>
      {queryMsg && <div className="message" onClick={() => setQueryMsg('')}>{queryMsg}</div>}
      <h4>{t('db.results')}</h4>
      {queryResult ? (
        <div>
          <p className="text-secondary" style={{ fontSize: '0.85em' }}>
            {queryResult.row_count} row(s)
          </p>
          <div style={{ overflowX: 'auto' }}>
            <table className="data-table">
              <thead>
                <tr>
                  {queryResult.columns.map((col, i) => <th key={i}>{col}</th>)}
                </tr>
              </thead>
              <tbody>
                {queryResult.rows.map((row, ri) => (
                  <tr key={ri}>
                    {row.map((cell, ci) => <td key={ci}>{cell}</td>)}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      ) : (
        <p className="text-secondary">{t('db.noResults')}</p>
      )}
    </div>
  );
}

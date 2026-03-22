import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { ProcessInfo } from '../types';
import { useT } from '../i18n';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

export default function ProcessesPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<ProcessInfo[]>('/api/processes', 5000);
  const [filter, setFilter] = useState('');
  const [sortBy, setSortBy] = useState<'cpu' | 'memory' | 'pid'>('cpu');
  const [message, setMessage] = useState('');

  const handleKill = async (pid: number, signal: string) => {
    const label = signal === 'KILL' ? 'Force kill' : 'Terminate';
    if (!window.confirm(`${label} PID ${pid}?`)) return;
    const res = await apiPost<string>('/api/processes/kill', { pid, signal });
    setMessage(res.success ? res.data || 'OK' : res.message);
    setTimeout(refetch, 1000);
  };

  if (loading) return <div className="panel loading">{t('processes.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  const sorted = [...(data || [])].sort((a, b) => {
    if (sortBy === 'cpu') return b.cpu - a.cpu;
    if (sortBy === 'memory') return b.memory - a.memory;
    return b.pid - a.pid;
  });

  const filtered = sorted.filter(p =>
    !filter || p.command.toLowerCase().includes(filter.toLowerCase())
      || p.user.toLowerCase().includes(filter.toLowerCase())
      || p.pid.toString().includes(filter)
  );

  return (
    <div className="panel">
      <h2>{t('processes.title')}</h2>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      <div className="filter-row">
        <input
          placeholder={t('processes.filterPlaceholder')}
          value={filter}
          onChange={e => setFilter(e.target.value)}
          className="filter-input"
        />
        <div className="btn-group">
          {(['cpu', 'memory', 'pid'] as const).map(s => (
            <button key={s} className={`btn btn-sm ${sortBy === s ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setSortBy(s)}>
              {s.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      <p className="text-secondary">{filtered.length} {t('processes.processes')}</p>

      <table className="data-table">
        <thead>
          <tr>
            <th>{t('processes.pid')}</th>
            <th>{t('processes.user')}</th>
            <th>{t('processes.cpuPercent')}</th>
            <th>{t('processes.memPercent')}</th>
            <th>{t('processes.rss')}</th>
            <th>{t('processes.started')}</th>
            <th>{t('processes.command')}</th>
            <th>{t('common.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {filtered.slice(0, 100).map(proc => (
            <tr key={proc.pid}>
              <td>{proc.pid}</td>
              <td><strong>{proc.user}</strong></td>
              <td>
                <span className={proc.cpu > 50 ? 'text-danger' : proc.cpu > 20 ? 'text-warning' : ''}>
                  {proc.cpu.toFixed(1)}
                </span>
              </td>
              <td>
                <span className={proc.memory > 50 ? 'text-danger' : proc.memory > 20 ? 'text-warning' : ''}>
                  {proc.memory.toFixed(1)}
                </span>
              </td>
              <td>{formatBytes(proc.rss * 1024)}</td>
              <td>{proc.started}</td>
              <td className="process-cmd"><code>{proc.command}</code></td>
              <td>
                <div className="btn-group">
                  <button className="btn btn-warning btn-sm"
                    onClick={() => handleKill(proc.pid, 'TERM')}>{t('processes.term')}</button>
                  <button className="btn btn-danger btn-sm"
                    onClick={() => handleKill(proc.pid, 'KILL')}>{t('processes.kill')}</button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {filtered.length > 100 && (
        <p className="text-secondary">{t('common.showingFirst', { count: filtered.length })}</p>
      )}
    </div>
  );
}

import React, { useState } from 'react';
import { useApi, apiPost } from '../shared/api/useApi';
import { ProcessInfo } from '../types';
import { useT } from '../i18n';
import { useSortable } from '../hooks/useSortable';

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
  const [message, setMessage] = useState('');
  const capability = useApi<{ can_signal: boolean }>('/api/processes/options');
  const [selected, setSelected] = useState<{ process: ProcessInfo; signal: string } | null>(null);
  const [confirmation, setConfirmation] = useState('');
  const [busy, setBusy] = useState(false);
  const canSignal = capability.data?.can_signal === true && !capability.error;
  const stale = selected && !data?.some(p => p.pid === selected.process.pid && p.identity === selected.process.identity);


  const filtered = (data || []).filter(p =>
    !filter || p.command.toLowerCase().includes(filter.toLowerCase())
      || p.user.toLowerCase().includes(filter.toLowerCase())
      || p.pid.toString().includes(filter)
  );

  const { sorted, toggle, indicator } = useSortable(filtered, 'cpu', 'desc');

  const handleKill = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!selected || stale || error || busy || !canSignal || confirmation !== String(selected.process.pid)) return;
    setBusy(true); setMessage('');
    try {
      const res = await apiPost<{ accepted: boolean; observation: string }>('/api/processes/kill', {
        pid: selected.process.pid, signal: selected.signal, expected_identity: selected.process.identity,
      });
      setMessage(res.success && res.data?.accepted ? t(res.data.observation === 'exited' ? 'processes.exited' : res.data.observation === 'still_present' ? 'processes.present' : 'processes.unknownObservation') : res.message || t('processes.unknownResult'));
    } catch { setMessage(t('processes.unknownResult')); }
    finally { setBusy(false); setSelected(null); setConfirmation(''); refetch(); }
  };

  if (loading) return <div className="panel loading">{t('processes.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;

  const S = (key: string, label: string) => (
    <th className="sortable" onClick={() => toggle(key)}>{label}{indicator(key)}</th>
  );

  return (
    <div className="panel">
      <h2>{t('processes.title')}</h2>
      <p>{t('processes.scopeHelp')}</p>
      {!canSignal && <p>{t('processes.readOnly')}</p>}
      {message && <div role="status" className="message">{message}</div>}
      {selected && <form className="inline-form" onSubmit={handleKill}>
        <p>PID {selected.process.pid} · UID {selected.process.user} · {selected.process.started}</p>
        <pre>{selected.process.command}</pre><p>{selected.signal === 'KILL' ? t('processes.killHelp') : t('processes.termHelp')}</p>
        {stale && <div role="alert">{t('processes.stale')}</div>}
        <label htmlFor="process-confirm">{t('processes.confirmPid')}</label>
        <input id="process-confirm" value={confirmation} onChange={e => setConfirmation(e.target.value)} disabled={busy} autoComplete="off" />
        <button className="btn btn-danger" type="submit" disabled={busy || !!stale || confirmation !== String(selected.process.pid) || !canSignal}>{t('processes.send')}</button>
        <button type="button" disabled={busy} onClick={() => { setSelected(null); setConfirmation(''); }}>{t('common.cancel')}</button>
      </form>}
      <div className="filter-row">
        <input placeholder={t('processes.filterPlaceholder')} value={filter}
          onChange={e => setFilter(e.target.value)} className="filter-input" />
      </div>
      <p className="text-secondary">{filtered.length} {t('processes.processes')}</p>
      <table className="data-table">
        <thead>
          <tr>
            {S('pid', t('processes.pid'))}
            {S('user', 'UID')}
            {S('cpu', t('processes.cpuPercent'))}
            {S('memory', t('processes.memPercent'))}
            {S('rss', t('processes.rss'))}
            {S('started', t('processes.started'))}
            {S('command', t('processes.command'))}
            <th>{t('common.actions')}</th>
          </tr>
        </thead>
        <tbody>
          {sorted.slice(0, 100).map(proc => (
            <tr key={proc.pid}>
              <td>{proc.pid}</td>
              <td><strong>{proc.user}</strong></td>
              <td><span className={proc.cpu > 50 ? 'text-danger' : proc.cpu > 20 ? 'text-warning' : ''}>{proc.cpu.toFixed(1)}</span></td>
              <td><span className={proc.memory > 50 ? 'text-danger' : proc.memory > 20 ? 'text-warning' : ''}>{proc.memory.toFixed(1)}</span></td>
              <td>{formatBytes(proc.rss * 1024)}</td>
              <td>{proc.started}</td>
              <td className="process-cmd"><code>{proc.command}</code></td>
              <td>
                <div className="btn-group">
                  {canSignal && proc.can_signal && <><button className="btn btn-warning btn-sm" disabled={busy} onClick={() => { setSelected({ process: { ...proc }, signal: 'TERM' }); setConfirmation(''); }}>{t('processes.term')}</button>
                  <button className="btn btn-danger btn-sm" disabled={busy} onClick={() => { setSelected({ process: { ...proc }, signal: 'KILL' }); setConfirmation(''); }}>{t('processes.kill')}</button></>}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {filtered.length > 100 && <p className="text-secondary">{t('common.showingFirst', { count: filtered.length })}</p>}
    </div>
  );
}

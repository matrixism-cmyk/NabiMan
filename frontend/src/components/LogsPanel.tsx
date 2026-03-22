import React, { useState, useEffect, useRef } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { LogEntry } from '../types';
import { useT } from '../i18n';

export default function LogsPanel() {
  const { t } = useT();
  const { data: units } = useApi<string[]>('/api/logs/units');
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [unit, setUnit] = useState('');
  const [lines, setLines] = useState(100);
  const [priority, setPriority] = useState('');
  const [loading, setLoading] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [filter, setFilter] = useState('');
  const logEndRef = useRef<HTMLDivElement>(null);

  const fetchLogs = async () => {
    setLoading(true);
    const body: Record<string, unknown> = { lines };
    if (unit) body.unit = unit;
    if (priority) body.priority = priority;
    const res = await apiPost<LogEntry[]>('/api/logs', body);
    if (res.success && res.data) setLogs(res.data);
    setLoading(false);
  };

  useEffect(() => {
    fetchLogs();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(fetchLogs, 5000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [autoRefresh, unit, priority, lines]);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [logs]);

  const filtered = logs.filter(l =>
    !filter || l.message.toLowerCase().includes(filter.toLowerCase())
      || l.unit.toLowerCase().includes(filter.toLowerCase())
  );

  return (
    <div className="panel">
      <h2>{t('logs.title')}</h2>

      <div className="filter-row">
        <select value={unit} onChange={e => setUnit(e.target.value)} className="select-input">
          <option value="">{t('logs.allUnits')}</option>
          {(units || []).map(u => (
            <option key={u} value={u}>{u}</option>
          ))}
        </select>
        <select value={priority} onChange={e => setPriority(e.target.value)} className="select-input">
          <option value="">{t('logs.allPriorities')}</option>
          <option value="emerg">{t('logs.emergency')}</option>
          <option value="alert">{t('logs.alert')}</option>
          <option value="crit">{t('logs.critical')}</option>
          <option value="err">{t('logs.error')}</option>
          <option value="warning">{t('logs.warning')}</option>
          <option value="notice">{t('logs.notice')}</option>
          <option value="info">{t('logs.info')}</option>
          <option value="debug">{t('logs.debug')}</option>
        </select>
        <select value={lines} onChange={e => setLines(Number(e.target.value))} className="select-input">
          <option value={50}>50 {t('logs.lines')}</option>
          <option value={100}>100 {t('logs.lines')}</option>
          <option value={200}>200 {t('logs.lines')}</option>
          <option value={500}>500 {t('logs.lines')}</option>
          <option value={1000}>1000 {t('logs.lines')}</option>
        </select>
        <button className="btn btn-primary" onClick={fetchLogs} disabled={loading}>
          {loading ? t('common.loading') : t('logs.fetch')}
        </button>
        <button
          className={`btn ${autoRefresh ? 'btn-success' : 'btn-secondary'}`}
          onClick={() => setAutoRefresh(!autoRefresh)}
        >
          {autoRefresh ? t('logs.autoOn') : t('logs.autoOff')}
        </button>
      </div>

      <input
        placeholder={t('logs.filterPlaceholder')}
        value={filter}
        onChange={e => setFilter(e.target.value)}
        className="filter-input"
      />

      <div className="log-container">
        {filtered.map((entry, i) => (
          <div key={i} className={`log-line ${
            entry.message.toLowerCase().includes('error') ? 'log-error' :
            entry.message.toLowerCase().includes('warn') ? 'log-warning' : ''
          }`}>
            <span className="log-timestamp">{entry.timestamp}</span>
            {entry.unit && <span className="log-unit">{entry.unit}</span>}
            <span className="log-message">{entry.message}</span>
          </div>
        ))}
        <div ref={logEndRef} />
      </div>
      <p className="text-secondary">{filtered.length} {t('logs.entries')}</p>
    </div>
  );
}

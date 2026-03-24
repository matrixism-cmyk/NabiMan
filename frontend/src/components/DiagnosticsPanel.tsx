import React, { useState } from 'react';
import { apiPost } from '../hooks/useApi';
import { PortCheckResult } from '../types';
import { useT } from '../i18n';

type Tool = 'ping' | 'traceroute' | 'nslookup' | 'port-check';

export default function DiagnosticsPanel() {
  const { t } = useT();
  const [tool, setTool] = useState<Tool>('ping');
  const [host, setHost] = useState('');
  const [port, setPort] = useState('80');
  const [output, setOutput] = useState('');
  const [running, setRunning] = useState(false);

  const handleRun = async () => {
    if (!host.trim()) return;
    setRunning(true);
    setOutput('');
    if (tool === 'port-check') {
      const res = await apiPost<PortCheckResult>('/api/diagnostics/port-check', { host, port: parseInt(port) });
      if (res.success && res.data) {
        setOutput(`${res.data.host}:${res.data.port} - ${res.data.open ? 'OPEN' : 'CLOSED'}`);
      } else { setOutput(res.message); }
    } else {
      const res = await apiPost<string>(`/api/diagnostics/${tool}`, { host });
      setOutput(res.data || res.message);
    }
    setRunning(false);
  };

  return (
    <div className="panel">
      <h2>{t('diagnostics.title')}</h2>
      <div className="filter-row">
        <select className="select-input" value={tool} onChange={e => setTool(e.target.value as Tool)}>
          <option value="ping">Ping</option>
          <option value="traceroute">Traceroute</option>
          <option value="nslookup">DNS Lookup</option>
          <option value="port-check">{t('diagnostics.portCheck')}</option>
        </select>
        <input
          className="filter-input"
          placeholder={t('diagnostics.hostPlaceholder')}
          value={host}
          onChange={e => setHost(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && handleRun()}
        />
        {tool === 'port-check' && (
          <input
            className="filter-input"
            style={{ maxWidth: 100 }}
            placeholder={t('diagnostics.port')}
            value={port}
            onChange={e => setPort(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleRun()}
          />
        )}
        <button className="btn btn-primary" onClick={handleRun} disabled={running}>
          {running ? t('common.running') : t('common.run')}
        </button>
      </div>
      {output && <pre className="log-output">{output}</pre>}
    </div>
  );
}

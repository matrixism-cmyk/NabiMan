import React from 'react';
import { useApi } from '../hooks/useApi';
import { useT } from '../i18n';

interface SwapInfo { filename: string; swap_type: string; size: number; used: number; priority: number; }
interface SwapStatus { total: number; used: number; free: number; swappiness: number; entries: SwapInfo[]; }

function fmtKB(kb: number): string {
  if (kb < 1024) return kb + ' KB';
  if (kb < 1048576) return (kb / 1024).toFixed(1) + ' MB';
  return (kb / 1048576).toFixed(1) + ' GB';
}

export default function SwapPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useApi<SwapStatus>('/api/swap/status', 10000);

  if (loading) return <div className="panel loading">{t('common.loading')}</div>;
  if (error) return <div className="panel error">{t('common.error')}: {error}</div>;
  if (!data) return null;

  const usePct = data.total > 0 ? (data.used / data.total * 100) : 0;
  const color = usePct > 80 ? '#e74c3c' : usePct > 50 ? '#f39c12' : '#2ecc71';

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('swap.title')}</h2>
        <button className="btn btn-secondary btn-sm" onClick={refetch}>{t('ssl.refresh')}</button>
      </div>
      <div className="info-grid">
        <div className="info-item"><span className="info-label">{t('swap.total')}</span><span className="info-value">{fmtKB(data.total)}</span></div>
        <div className="info-item"><span className="info-label">{t('swap.used')}</span><span className="info-value">{fmtKB(data.used)}</span></div>
        <div className="info-item"><span className="info-label">{t('swap.free')}</span><span className="info-value">{fmtKB(data.free)}</span></div>
        <div className="info-item"><span className="info-label">{t('swap.swappiness')}</span><span className="info-value">{data.swappiness}</span></div>
      </div>
      {data.total > 0 && (
        <div className="progress-section">
          <div className="progress-label"><span>Swap</span><span>{usePct.toFixed(1)}%</span></div>
          <div className="progress-bar"><div className="progress-fill" style={{ width: `${usePct}%`, backgroundColor: color }} /></div>
        </div>
      )}
      {data.entries.length > 0 && (
        <table className="data-table" style={{ marginTop: 16 }}>
          <thead><tr><th>{t('swap.device')}</th><th>{t('swap.type')}</th><th>{t('swap.size')}</th><th>{t('swap.used')}</th><th>{t('swap.priority')}</th></tr></thead>
          <tbody>
            {data.entries.map((e, i) => (
              <tr key={i}><td><code>{e.filename}</code></td><td>{e.swap_type}</td><td>{fmtKB(e.size)}</td><td>{fmtKB(e.used)}</td><td>{e.priority}</td></tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

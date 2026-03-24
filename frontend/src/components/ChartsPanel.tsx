import React, { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { useT } from '../i18n';

interface DataPoint { timestamp: number; cpu: number; memory_pct: number; rx_bytes_sec: number; tx_bytes_sec: number; }

function MiniChart({ data, valueKey, color, label, format }: {
  data: DataPoint[]; valueKey: keyof DataPoint; color: string; label: string; format: (v: number) => string;
}) {
  if (data.length < 2) return <div className="text-secondary">Collecting data...</div>;
  const values = data.map(d => d[valueKey] as number);
  const max = Math.max(...values, 1);
  const w = 100 / values.length;
  const latest = values[values.length - 1];

  return (
    <div className="chart-box">
      <div className="chart-header"><span>{label}</span><span className="chart-value">{format(latest)}</span></div>
      <svg viewBox="0 0 100 30" preserveAspectRatio="none" className="chart-svg">
        <polyline
          fill="none" stroke={color} strokeWidth="0.5"
          points={values.map((v, i) => `${i * w},${30 - (v / max) * 28}`).join(' ')}
        />
        <polyline
          fill={color} fillOpacity="0.1" stroke="none"
          points={`0,30 ${values.map((v, i) => `${i * w},${30 - (v / max) * 28}`).join(' ')} ${(values.length - 1) * w},30`}
        />
      </svg>
    </div>
  );
}

function fmtBytes(b: number): string {
  if (b < 1024) return b + ' B/s';
  if (b < 1048576) return (b / 1024).toFixed(1) + ' KB/s';
  return (b / 1048576).toFixed(1) + ' MB/s';
}

export default function ChartsPanel() {
  const { t } = useT();
  const [hours, setHours] = useState(1);
  const { data, loading } = useApi<DataPoint[]>(`/api/server/history?hours=${hours}`, 10000);

  if (loading && !data) return <div className="panel loading">{t('common.loading')}</div>;
  const pts = data || [];

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>{t('charts.title')}</h2>
        <div className="btn-group">
          {[1, 6, 24].map(h => (
            <button key={h} className={`btn btn-sm ${hours === h ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setHours(h)}>{h}h</button>
          ))}
        </div>
      </div>
      <div className="charts-grid">
        <MiniChart data={pts} valueKey="cpu" color="#3b82f6" label="CPU" format={v => v.toFixed(1) + '%'} />
        <MiniChart data={pts} valueKey="memory_pct" color="#2ecc71" label={t('charts.memory')} format={v => v.toFixed(1) + '%'} />
        <MiniChart data={pts} valueKey="rx_bytes_sec" color="#f39c12" label="RX" format={fmtBytes} />
        <MiniChart data={pts} valueKey="tx_bytes_sec" color="#e74c3c" label="TX" format={fmtBytes} />
      </div>
    </div>
  );
}

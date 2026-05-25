import React from 'react';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { ErrorBanner, PanelLayout, StatusBadge } from './common';

interface HealthEntry {
  connected: boolean;
  endpoint: string;
  latency_ms?: number | null;
  error?: string | null;
}

interface HealthSummary {
  kubernetes: HealthEntry;
  rancher: HealthEntry;
  axgate: HealthEntry;
  harbor: HealthEntry;
  mode: string;
}

function isMock(endpoint: string): boolean {
  return endpoint.startsWith('mock://');
}

function latencyTone(ms: number | null | undefined): 'success' | 'warning' | 'error' {
  if (ms === null || ms === undefined) return 'error';
  if (ms > 1000) return 'error';
  if (ms > 300) return 'warning';
  return 'success';
}

function StatusCard({
  label,
  entry,
  icon,
}: {
  label: string;
  entry: HealthEntry;
  icon: React.ReactNode;
}) {
  const mock = isMock(entry.endpoint);
  const color = !entry.connected ? '#ef4444' : mock ? '#9ca3af' : '#10b981';
  return (
    <div
      style={{
        background: 'white',
        borderRadius: '8px',
        border: '1px solid #e5e7eb',
        borderLeft: `3px solid ${color}`,
        padding: '16px',
        display: 'flex',
        flexDirection: 'column',
        gap: '10px',
      }}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
        }}
      >
        <h3
          style={{
            margin: 0,
            fontSize: '14px',
            fontWeight: 600,
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
          }}
        >
          <span style={{ color, fontSize: '16px' }}>{icon}</span>
          {label}
        </h3>
        <StatusBadge tone={mock ? 'neutral' : entry.connected}>
          {mock ? 'Mock' : entry.connected ? 'connected' : 'disconnected'}
        </StatusBadge>
      </div>
      <div style={{ fontSize: '12px', color: '#6b7280' }}>
        <div>
          <span style={{ color: '#9ca3af' }}>endpoint:</span>{' '}
          <code style={{ fontSize: '11px' }}>{entry.endpoint}</code>
        </div>
        {entry.latency_ms !== null && entry.latency_ms !== undefined && (
          <div style={{ marginTop: '4px' }}>
            <span style={{ color: '#9ca3af' }}>latency:</span>{' '}
            <StatusBadge tone={latencyTone(entry.latency_ms)}>
              {entry.latency_ms}ms
            </StatusBadge>
          </div>
        )}
        {entry.error && (
          <div
            style={{
              marginTop: '6px',
              color: '#991b1b',
              background: '#fee2e2',
              padding: '4px 8px',
              borderRadius: '4px',
              fontSize: '11px',
            }}
          >
            {entry.error}
          </div>
        )}
      </div>
    </div>
  );
}

export default function MecHealthPanel() {
  const { data, loading, error, refetch } = useMecApi<HealthSummary>(
    '/api/mec/v1/health/summary',
    30_000,
  );

  const connected = data
    ? [data.kubernetes, data.rancher, data.axgate, data.harbor].filter(
        (e) => e.connected && !isMock(e.endpoint),
      ).length
    : 0;

  return (
    <PanelLayout
      title="외부 시스템 연결 상태"
      subtitle={
        data ? `모드: ${data.mode} · 실제 연결 ${connected} / 4` : '검사 중...'
      }
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          재검사
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />
      {loading && !data && <div style={{ color: '#6b7280' }}>검사 중...</div>}

      {data && (
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
            gap: '12px',
          }}
        >
          <StatusCard label="Kubernetes API" entry={data.kubernetes} icon="⎈" />
          <StatusCard label="Rancher" entry={data.rancher} icon="⚓" />
          <StatusCard label="AXGATE Firewall" entry={data.axgate} icon="🛡" />
          <StatusCard label="Harbor Registry" entry={data.harbor} icon="📦" />
        </div>
      )}
    </PanelLayout>
  );
}

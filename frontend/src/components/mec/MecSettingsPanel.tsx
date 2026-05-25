import React, { useState } from 'react';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { ThemeSelector } from '../../theme';

interface KubeSettings {
  kubeconfig_path?: string | null;
  namespace: string;
  configured: boolean;
}

interface EndpointSettings {
  url?: string | null;
  username_configured: boolean;
  secret_configured: boolean;
  insecure_tls: boolean;
  configured: boolean;
}

interface AxgateSettings {
  host?: string | null;
  port: number;
  username_configured: boolean;
  password_configured: boolean;
  configured: boolean;
}

interface Snapshot {
  mode: string;
  read_only: boolean;
  skip_confirm: boolean;
  kubernetes: KubeSettings;
  rancher: EndpointSettings;
  axgate: AxgateSettings;
  harbor: EndpointSettings;
  systemd_dropin_sample: string;
}

function Badge({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span
      style={{
        padding: '2px 8px',
        borderRadius: '10px',
        fontSize: '11px',
        background: ok ? '#d1fae5' : '#fee2e2',
        color: ok ? '#065f46' : '#991b1b',
      }}
    >
      {ok ? '✓ ' + label : '✗ ' + label}
    </span>
  );
}

function Row({
  label,
  value,
  badges,
}: {
  label: string;
  value?: React.ReactNode;
  badges: React.ReactNode;
}) {
  return (
    <div
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: '8px 0',
        borderBottom: '1px solid #e5e7eb',
      }}
    >
      <div>
        <div style={{ fontWeight: 500 }}>{label}</div>
        <div style={{ fontSize: '12px', color: '#6b7280', fontFamily: 'monospace' }}>
          {value || '-'}
        </div>
      </div>
      <div style={{ display: 'flex', gap: '4px' }}>{badges}</div>
    </div>
  );
}

export default function MecSettingsPanel() {
  const { data, loading, error, refetch } = useMecApi<Snapshot>(
    '/api/mec/v1/settings/snapshot',
    60_000,
  );
  const [copied, setCopied] = useState(false);

  const copyDropin = () => {
    if (!data) return;
    navigator.clipboard.writeText(data.systemd_dropin_sample);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="panel">
      <div className="panel-header">
        <h2>MEC 연동 설정</h2>
        <button className="btn btn-secondary" onClick={refetch}>
          새로고침
        </button>
      </div>

      {error && <div className="panel-error">{error}</div>}
      {loading && !data && <div className="panel-loading">로딩 중...</div>}

      {data && (
        <>
          {data.read_only && (
            <div
              className="panel-card"
              style={{
                background: '#fef3c7',
                borderLeft: '4px solid #f59e0b',
                marginBottom: '16px',
              }}
            >
              <strong>🔒 Read-only 모드가 켜져 있습니다.</strong> 모든 변경 작업이 차단됩니다.
              안전한 첫 연결을 위해 권장되는 상태이며, 조회가 모두 정상이면{' '}
              <code>NABIMAN_MEC_READ_ONLY=false</code> 로 변경 후 재시작하세요.
            </div>
          )}
          <div className="panel-card" style={{ marginBottom: '16px' }}>
            <h3 style={{ marginTop: 0 }}>
              현재 설정 ({data.mode} 모드
              {data.read_only ? ', 🔒 read-only' : ''}
              {data.skip_confirm ? ', confirm skipped' : ''})
            </h3>
            <Row
              label="Kubernetes"
              value={data.kubernetes.kubeconfig_path || '(in-cluster 추론)'}
              badges={
                <>
                  <Badge ok={data.kubernetes.configured} label="configured" />
                </>
              }
            />
            <Row
              label="Rancher"
              value={data.rancher.url}
              badges={
                <>
                  <Badge ok={!!data.rancher.url} label="URL" />
                  <Badge ok={data.rancher.secret_configured} label="Token" />
                </>
              }
            />
            <Row
              label="AXGATE"
              value={
                data.axgate.host
                  ? `${data.axgate.host}:${data.axgate.port}`
                  : undefined
              }
              badges={
                <>
                  <Badge ok={!!data.axgate.host} label="Host" />
                  <Badge ok={data.axgate.username_configured} label="User" />
                  <Badge ok={data.axgate.password_configured} label="Pass" />
                </>
              }
            />
            <Row
              label="Harbor"
              value={data.harbor.url}
              badges={
                <>
                  <Badge ok={!!data.harbor.url} label="URL" />
                  <Badge ok={data.harbor.username_configured} label="User" />
                  <Badge ok={data.harbor.secret_configured} label="Pass" />
                </>
              }
            />
          </div>

          <div
            className="panel-card"
            style={{ marginBottom: '16px' }}
          >
            <h3 style={{ marginTop: 0 }}>색상 테마</h3>
            <p style={{ color: '#6b7280', fontSize: '13px', marginBottom: '12px' }}>
              클릭으로 즉시 전환됩니다. 선택한 테마는 브라우저에 저장되어 다음 로그인에도 유지됩니다.
              (상단 헤더의 셀렉트 박스에서도 동일하게 변경 가능)
            </p>
            <ThemeSelector />
          </div>

          <div className="panel-card">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h3 style={{ margin: 0 }}>systemd drop-in 템플릿</h3>
              <button className="btn btn-secondary btn-small" onClick={copyDropin}>
                {copied ? '복사됨!' : '복사'}
              </button>
            </div>
            <p style={{ color: '#6b7280', fontSize: '13px' }}>
              아래 내용을 <code>/etc/systemd/system/nabiman.service.d/mec.conf</code> 에 저장한 뒤
              적용 명령을 실행하세요. 비밀값(*** 부분)은 실제 값으로 교체해야 합니다.
            </p>
            <pre
              style={{
                background: '#0f172a',
                color: '#e2e8f0',
                padding: '12px',
                borderRadius: '6px',
                fontSize: '12px',
                overflow: 'auto',
                maxHeight: '400px',
              }}
            >
              {data.systemd_dropin_sample}
            </pre>
          </div>
        </>
      )}
    </div>
  );
}

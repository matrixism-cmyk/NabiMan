import React, { useState } from 'react';
import { mecPost, mecDelete } from '../../hooks/mec/useMecApi';

interface Props {
  tenantId: string;
  onChanged: () => void;
}

interface DeployResult {
  ssh_user: string;
  ssh_password: string;
  vscode_password: string;
  message: string;
}

export default function TenantStarterKitTab({ tenantId, onChanged }: Props) {
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [result, setResult] = useState<DeployResult | null>(null);

  const deploy = async () => {
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost<DeployResult>(
        `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/starter-kit`,
        {},
      );
      if (res.error) {
        setErr(res.error.message);
      } else if (res.data) {
        setResult(res.data);
        onChanged();
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    const typed = window.prompt(
      `⚠️ Starter Kit (Ubuntu SSH + VS Code) 을(를) 삭제합니다.\n확인을 위해 테넌트 ID '${tenantId}' 를 다시 입력하세요:`,
      '',
    );
    if (typed !== tenantId) {
      if (typed !== null) {
        setErr('입력한 ID가 일치하지 않아 삭제를 취소했습니다.');
      }
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      await mecDelete(
        `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/starter-kit`,
        tenantId,
      );
      setResult(null);
      onChanged();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel-card">
      <h3 style={{ marginTop: 0 }}>Starter Kit</h3>
      <p style={{ color: '#6b7280', fontSize: '14px' }}>
        Ubuntu SSH (port 22) + VS Code (code-server, port 8080) 파드를 이 테넌트에 배포합니다.
        MetalLB 가 설정되어 있다면 LoadBalancer 서비스로 자동 외부 IP가 할당됩니다.
      </p>

      {err && <div className="panel-error">{err}</div>}

      {result && (
        <div
          className="panel-card"
          style={{
            background: '#fef3c7',
            borderColor: '#f59e0b',
            marginTop: '12px',
            marginBottom: '12px',
          }}
        >
          <h4 style={{ marginTop: 0 }}>배포 완료 — 비밀번호는 한 번만 표시됩니다</h4>
          <div style={{ fontFamily: 'monospace', fontSize: '14px' }}>
            <div>SSH user: <strong>{result.ssh_user}</strong></div>
            <div>SSH password: <strong>{result.ssh_password}</strong></div>
            <div>VS Code password: <strong>{result.vscode_password}</strong></div>
          </div>
          <p style={{ fontSize: '12px', color: '#92400e', marginBottom: 0 }}>
            이 비밀번호는 저장되지 않습니다. 안전한 곳에 복사해두세요.
          </p>
        </div>
      )}

      <div style={{ display: 'flex', gap: '8px' }}>
        <button
          className="btn btn-primary"
          onClick={deploy}
          disabled={busy}
        >
          {busy ? '처리 중...' : '배포 / 재배포'}
        </button>
        <button
          className="btn btn-danger"
          onClick={remove}
          disabled={busy}
        >
          삭제
        </button>
      </div>
    </div>
  );
}

import React, { useState } from 'react';
import { useT } from '../../i18n';
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
  const { t } = useT();
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
      t('mec.tenant.starter.deleteConfirm', { id: tenantId }),
      '',
    );
    if (typed !== tenantId) {
      if (typed !== null) {
        setErr(t('mec.tenant.starter.deleteMismatch'));
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
        {t('mec.tenant.starter.description')}
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
          <h4 style={{ marginTop: 0 }}>{t('mec.tenant.starter.deployedHeading')}</h4>
          <div style={{ fontFamily: 'monospace', fontSize: '14px' }}>
            <div>SSH user: <strong>{result.ssh_user}</strong></div>
            <div>SSH password: <strong>{result.ssh_password}</strong></div>
            <div>VS Code password: <strong>{result.vscode_password}</strong></div>
          </div>
          <p style={{ fontSize: '12px', color: '#92400e', marginBottom: 0 }}>
            {t('mec.tenant.starter.passwordWarning')}
          </p>
        </div>
      )}

      <div style={{ display: 'flex', gap: '8px' }}>
        <button
          className="btn btn-primary"
          onClick={deploy}
          disabled={busy}
        >
          {busy ? t('mec.tenant.starter.processing') : t('mec.tenant.starter.deploy')}
        </button>
        <button
          className="btn btn-danger"
          onClick={remove}
          disabled={busy}
        >
          {t('mec.tenant.starter.delete')}
        </button>
      </div>
    </div>
  );
}

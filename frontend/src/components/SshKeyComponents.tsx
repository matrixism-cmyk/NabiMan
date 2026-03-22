import React, { useState } from 'react';
import { useApi, apiPost } from '../hooks/useApi';
import { RemoteServer, SshKeyInfo } from '../types';
import { useT } from '../i18n';

export function SshKeyPanel() {
  const { t } = useT();
  const { data: keyInfo, loading, refetch } = useApi<SshKeyInfo>('/api/remote-servers/ssh-key');
  const [generating, setGenerating] = useState(false);
  const [message, setMessage] = useState('');
  const [copied, setCopied] = useState(false);

  const handleGenerate = async () => {
    setGenerating(true);
    const res = await apiPost<SshKeyInfo>('/api/remote-servers/ssh-key/generate', {});
    setMessage(res.success ? t('sshKey.generated') : res.message);
    refetch();
    setGenerating(false);
  };

  const handleCopy = () => {
    if (keyInfo?.public_key) {
      navigator.clipboard.writeText(keyInfo.public_key);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  if (loading) return <div className="loading">{t('common.loading')}</div>;

  return (
    <div className="config-section ssh-key-section">
      <div className="config-header">
        <h3>{t('sshKey.title')}</h3>
        <div className="config-meta">
          <span className={`status-badge ${keyInfo?.exists ? 'up' : 'down'}`}>
            {keyInfo?.exists ? t('sshKey.exists') : t('sshKey.noKey')}
          </span>
        </div>
        <div className="btn-group">
          {!keyInfo?.exists && (
            <button className="btn btn-primary btn-sm" onClick={handleGenerate} disabled={generating}>
              {generating ? t('sshKey.generating') : t('sshKey.generate')}
            </button>
          )}
        </div>
      </div>

      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {keyInfo?.exists && keyInfo.public_key && (
        <div style={{ padding: '8px 0' }}>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '4px' }}>
            <label style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{t('sshKey.publicKey')}</label>
            <button className="btn btn-secondary btn-sm" onClick={handleCopy}>
              {copied ? t('sshKey.copied') : t('sshKey.copy')}
            </button>
          </div>
          <pre className="config-preview" style={{ fontSize: '11px', maxHeight: '60px', wordBreak: 'break-all', whiteSpace: 'pre-wrap' }}>
            {keyInfo.public_key}
          </pre>
          {keyInfo.fingerprint && (
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '4px' }}>
              {t('sshKey.fingerprint')}: <code>{keyInfo.fingerprint}</code>
            </div>
          )}
          <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginTop: '4px' }}>
            {t('sshKey.path')}: <code>{keyInfo.key_path}</code>
          </div>
          <p style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '8px' }}>
            {t('sshKey.deployHint')}
          </p>
        </div>
      )}

      {!keyInfo?.exists && (
        <p className="text-secondary">{t('sshKey.noKeyHint')}</p>
      )}
    </div>
  );
}

export function DeployKeyDialog({ server, onClose }: { server: RemoteServer; onClose: () => void }) {
  const { t } = useT();
  const [password, setPassword] = useState('');
  const [deploying, setDeploying] = useState(false);
  const [result, setResult] = useState('');
  const [success, setSuccess] = useState(false);
  const [testing, setTesting] = useState(false);
  const [keyWorks, setKeyWorks] = useState<boolean | null>(null);

  const handleDeploy = async () => {
    if (!password) return;
    setDeploying(true);
    setResult('');
    const res = await apiPost<string>(`/api/remote-servers/${server.id}/deploy-key`, { password });
    setResult(res.success ? (res.data || 'Key deployed') : res.message);
    setSuccess(res.success);
    setDeploying(false);
    if (res.success) setPassword('');
  };

  const handleTest = async () => {
    setTesting(true);
    const res = await apiPost<boolean>(`/api/remote-servers/${server.id}/test-key`, {});
    setKeyWorks(res.success ? res.data : false);
    setTesting(false);
  };

  return (
    <div className="deploy-key-dialog">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
        <strong>{t('deployKey.title')} {server.name} ({server.user}@{server.host})</strong>
        <button className="btn btn-secondary btn-sm" onClick={onClose}>{t('common.close')}</button>
      </div>

      <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '8px' }}>
        <button className="btn btn-secondary btn-sm" onClick={handleTest} disabled={testing}>
          {testing ? t('deployKey.testing') : t('deployKey.testAuth')}
        </button>
        {keyWorks !== null && (
          <span className={`status-badge ${keyWorks ? 'up' : 'down'}`}>
            {keyWorks ? t('deployKey.authWorks') : t('deployKey.authNotWorking')}
          </span>
        )}
      </div>

      {keyWorks !== true && (
        <>
          <p style={{ fontSize: '12px', color: 'var(--text-secondary)', margin: '8px 0' }}>
            <code>{server.user}@{server.host}</code>{t('deployKey.passwordHint')}
          </p>
          <div className="filter-row">
            <input
              type="password" value={password}
              onChange={e => setPassword(e.target.value)}
              placeholder={t('deployKey.passwordPlaceholder')}
              className="filter-input" style={{ flex: 1 }}
              onKeyDown={e => e.key === 'Enter' && handleDeploy()}
            />
            <button className="btn btn-primary btn-sm" onClick={handleDeploy} disabled={deploying || !password}>
              {deploying ? t('deployKey.deploying') : t('deployKey.deploy')}
            </button>
          </div>
        </>
      )}

      {result && (
        <pre className="config-preview" style={{
          marginTop: '8px', maxHeight: '100px', fontSize: '12px',
          borderColor: success ? 'var(--success)' : 'var(--danger)', whiteSpace: 'pre-wrap',
        }}>
          {result}
        </pre>
      )}
    </div>
  );
}

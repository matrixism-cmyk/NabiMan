import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../../types';
import { useT } from '../../i18n';
import { DeployKeyDialog } from '../SshKeyComponents';
import ServerForm from './ServerForm';

/** Live facts about one server: probe it and show what came back. */
export function ServerStatusView({ server }: { server: RemoteServer }) {
  const { t } = useT();
  const [status, setStatus] = useState<RemoteServerStatus | null>(null);
  const [checking, setChecking] = useState(false);

  const check = async () => {
    setChecking(true);
    const res = await apiPost<RemoteServerStatus>(`/api/remote-servers/${server.id}/check`, {});
    if (res.success && res.data) setStatus(res.data);
    setChecking(false);
  };

  return (
    <div className="rw-section">
      <div className="rw-section-head">
        <h3>{t('remote.statusView')}</h3>
        <button className="btn btn-primary btn-sm" onClick={check} disabled={checking}>
          {checking ? t('remote.checking') : t('remote.check')}
        </button>
      </div>
      {!status && (
        <p className="text-secondary">
          {server.last_checked
            ? `${t('remote.lastChecked')}: ${server.last_checked} · ${server.status || t('common.unknown')}`
            : t('remote.checkHint')}
        </p>
      )}
      {status && (
        <div className="server-status-grid">
          <div className="status-item"><label>{t('remote.hostname')}</label><span>{status.hostname}</span></div>
          <div className="status-item"><label>{t('remote.os')}</label><span>{status.os}</span></div>
          <div className="status-item"><label>{t('remote.uptime')}</label><span>{status.uptime}</span></div>
          <div className="status-item"><label>{t('remote.cpu')}</label><span>{status.cpu_usage}</span></div>
          <div className="status-item"><label>{t('remote.memory')}</label><span>{status.memory}</span></div>
          <div className="status-item"><label>{t('remote.diskRoot')}</label><span>{status.disk}</span></div>
          <div className="status-item"><label>{t('remote.load')}</label><span>{status.load}</span></div>
          <div className="status-item"><label>{t('remote.checked')}</label><span>{status.checked_at}</span></div>
        </div>
      )}
    </div>
  );
}

/** Run one command on the server without opening a shell. */
export function ServerExecView({ server }: { server: RemoteServer }) {
  const { t } = useT();
  const [command, setCommand] = useState('');
  const [result, setResult] = useState('');
  const [running, setRunning] = useState(false);

  const run = async () => {
    if (!command.trim()) return;
    setRunning(true);
    const res = await apiPost<string>(`/api/remote-servers/${server.id}/exec`, { command });
    setResult(res.success ? (res.data || '') : `${t('common.error')}: ${res.message}`);
    setRunning(false);
  };

  return (
    <div className="rw-section">
      <div className="rw-section-head"><h3>{t('remote.exec')}</h3></div>
      <div className="filter-row">
        <input value={command} onChange={e => setCommand(e.target.value)} className="filter-input"
          placeholder={t('remote.execPlaceholder')} style={{ flex: 1 }}
          onKeyDown={e => e.key === 'Enter' && run()} />
        <button className="btn btn-primary btn-sm" onClick={run} disabled={running}>
          {running ? t('common.running') : t('common.run')}
        </button>
      </div>
      {result && <pre className="config-preview">{result}</pre>}
    </div>
  );
}

interface SettingsProps {
  server: RemoteServer;
  onChanged: () => void;
  onDeleted: () => void;
}

/** Everything about the record itself: fields, key deployment, removal. */
export function ServerSettingsView({ server, onChanged, onDeleted }: SettingsProps) {
  const { t } = useT();
  const [showDeploy, setShowDeploy] = useState(false);

  const remove = async () => {
    if (!window.confirm(t('remote.deleteConfirm', { name: server.name }))) return;
    await apiRequest(`/api/remote-servers/${server.id}`, { method: 'DELETE' });
    onDeleted();
  };

  return (
    <div className="rw-section">
      <div className="rw-section-head">
        <h3>{t('remote.editServer')}</h3>
        <div className="btn-group">
          <button className="btn btn-secondary btn-sm" onClick={() => setShowDeploy(v => !v)}>
            {showDeploy ? t('remote.hideKey') : t('remote.deployKey')}
          </button>
          <button className="btn btn-danger btn-sm" onClick={remove}>{t('common.delete')}</button>
        </div>
      </div>
      {showDeploy && <DeployKeyDialog server={server} onClose={() => setShowDeploy(false)} />}
      <ServerForm key={server.id} server={server} onSaved={onChanged} />
    </div>
  );
}

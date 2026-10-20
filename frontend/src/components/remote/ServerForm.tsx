import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { RemoteServer, RemoteServerStatus } from '../../types';
import { useT } from '../../i18n';
import { FileProtocol, PLAINTEXT, protocolName, TerminalProtocol } from './protocols';

interface Props {
  /** Absent for a new server, present when editing an existing one. */
  server?: RemoteServer;
  onSaved: (server: RemoteServer) => void;
  onCancel?: () => void;
}

/** A new server's settings view looks up which protocols work, once. */
export const DETECT_PENDING_KEY = 'nabiman_detect_pending';

/**
 * The one form used both to add a server and to edit it — every field the
 * server record has, so nothing is editable in one place and frozen in the
 * other. How the terminal and the files are reached are chosen separately:
 * SSH-only, SFTP-only and FTP-only servers are all common.
 */
export default function ServerForm({ server, onSaved, onCancel }: Props) {
  const { t } = useT();
  const [name, setName] = useState(server?.name || '');
  const [host, setHost] = useState(server?.host || '');
  const [terminal, setTerminal] = useState<TerminalProtocol>(server?.terminal || 'ssh');
  const [files, setFiles] = useState<FileProtocol>(server?.files || 'sftp');
  const [port, setPort] = useState(String(server?.port || 22));
  const [ftpPort, setFtpPort] = useState(String(server?.ftp_port || 21));
  const [fingerprint, setFingerprint] = useState(server?.tls_fingerprint || '');
  const [implicit, setImplicit] = useState(Boolean(server?.ftps_implicit));
  const [user, setUser] = useState(server?.user || 'root');
  const [auth, setAuth] = useState(server?.auth_method || 'key');
  const [password, setPassword] = useState('');
  const [tags, setTags] = useState(server?.tags.join(', ') || '');
  const [memo, setMemo] = useState(server?.memo || '');
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);

  const ssh = terminal === 'ssh' || files === 'sftp';
  const ftp = files === 'ftp' || files === 'ftps';
  // Keys are an SSH thing; Telnet and FTP always log in with a password.
  const method = ssh ? auth : 'password';
  const plain = [terminal, files].filter(p => PLAINTEXT.has(p)).map(protocolName);

  // Implicit FTPS is FTPS with TLS from the first byte; one choice in the list.
  const chooseFiles = (value: string) => {
    const nextImplicit = value === 'ftps-implicit';
    if (nextImplicit && ftpPort === '21') setFtpPort('990');
    if (!nextImplicit && ftpPort === '990') setFtpPort('21');
    setImplicit(nextImplicit);
    setFiles((nextImplicit ? 'ftps' : value) as FileProtocol);
  };

  const chooseTerminal = (next: TerminalProtocol) => {
    // Follow the protocol's usual port unless one was typed in.
    if (port === '22' && next === 'telnet') setPort('23');
    if (port === '23' && next !== 'telnet') setPort('22');
    setTerminal(next);
  };

  /** Prove a freshly saved password authenticates instead of finding out later. */
  const verifyPassword = async (id: string) => {
    setMessage(t('remote.passwordVerifying'));
    const res = await apiPost<RemoteServerStatus>(`/api/remote-servers/${id}/check`, {});
    const ok = res.success && res.data?.status === 'online';
    setMessage(ok ? t('remote.passwordVerified')
      : `${t('remote.passwordVerifyFailed')} ${res.data?.hostname || res.message}`);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const portNum = parseInt(port, 10);
    const ftpPortNum = parseInt(ftpPort, 10);
    const validPort = (n: number) => n >= 1 && n <= 65535;
    if (!host.trim()) { setMessage(t('remote.hostRequired')); return; }
    if (!user.trim()) { setMessage(t('remote.userRequired')); return; }
    if (terminal === 'none' && files === 'none') { setMessage(t('remote.proto.needOne')); return; }
    if (terminal === 'telnet' && files === 'sftp') { setMessage(t('remote.proto.telnetSftp')); return; }
    if (!validPort(portNum) || (ftp && !validPort(ftpPortNum))) { setMessage(t('remote.portInvalid')); return; }

    setSaving(true);
    const body: Record<string, unknown> = {
      name: name.trim() || host.trim(),
      host: host.trim(), port: portNum, user: user.trim(),
      auth_method: method, memo: memo.trim(),
      tags: tags.split(',').map(s => s.trim()).filter(Boolean),
      terminal, files, ftp_port: ftp ? ftpPortNum : server?.ftp_port || 21,
      tls_fingerprint: files === 'ftps' ? fingerprint.trim() : '',
      ftps_implicit: files === 'ftps' && implicit,
    };
    // On edit an empty box keeps the stored password; on add it means none.
    if (password || !server) body.password = method === 'password' ? password : '';

    const res = server
      ? await apiRequest<RemoteServer>(`/api/remote-servers/${server.id}`, {
          method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
        })
      : await apiPost<RemoteServer>('/api/remote-servers', body);
    setSaving(false);

    if (!res.success || !res.data) { setMessage(res.message || t('common.error')); return; }
    setPassword('');
    if (!server) {
      try { sessionStorage.setItem(DETECT_PENDING_KEY, res.data.id); } catch { /* private mode */ }
    }
    onSaved(res.data);
    if (ssh && !res.data.host_trust) setMessage(t('remote.trust.unverified'));
    else if (password) verifyPassword(res.data.id);
    else setMessage(server ? t('remote.serverSaved') : t('remote.serverAdded'));
  };

  const clearPassword = async () => {
    if (!server || !window.confirm(t('remote.clearPasswordConfirm'))) return;
    const res = await apiRequest<RemoteServer>(`/api/remote-servers/${server.id}`, {
      method: 'PUT', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ password: '', auth_method: ssh ? 'key' : 'password' }),
    });
    if (res.success && res.data) { if (ssh) setAuth('key'); onSaved(res.data); }
  };

  return (
    <form className="rw-form" onSubmit={handleSubmit}>
      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}
      <div className="form-grid">
        <div className="form-group"><label>{t('common.name')}</label>
          <input value={name} onChange={e => setName(e.target.value)} placeholder="web-prod-01" className="filter-input" /></div>
        <div className="form-group"><label>{t('remote.host')} *</label>
          <input value={host} onChange={e => setHost(e.target.value)} placeholder="192.168.1.100" required className="filter-input" /></div>
        <div className="form-group"><label htmlFor="srv-terminal">{t('remote.proto.terminal')}</label>
          <select id="srv-terminal" value={terminal} onChange={e => chooseTerminal(e.target.value as TerminalProtocol)} className="select-input">
            <option value="ssh">SSH</option>
            <option value="telnet">{t('remote.proto.telnetOption')}</option>
            <option value="none">{t('remote.proto.none')}</option>
          </select></div>
        <div className="form-group"><label htmlFor="srv-files">{t('remote.proto.files')}</label>
          <select id="srv-files" value={files === 'ftps' && implicit ? 'ftps-implicit' : files}
            onChange={e => chooseFiles(e.target.value)} className="select-input">
            <option value="sftp">SFTP</option>
            <option value="ftps">{t('remote.proto.ftpsOption')}</option>
            <option value="ftps-implicit">{t('remote.proto.ftpsImplicitOption')}</option>
            <option value="ftp">{t('remote.proto.ftpOption')}</option>
            <option value="none">{t('remote.proto.none')}</option>
          </select></div>
        {(terminal !== 'none' || files === 'sftp') && (
          <div className="form-group"><label htmlFor="srv-port">{terminal === 'telnet' ? t('remote.proto.telnetPort') : t('remote.proto.sshPort')}</label>
            <input id="srv-port" value={port} onChange={e => setPort(e.target.value)} type="number" min={1} max={65535} className="filter-input" /></div>
        )}
        {ftp && (
          <div className="form-group"><label htmlFor="srv-ftp-port">{t('remote.proto.ftpPort')}</label>
            <input id="srv-ftp-port" value={ftpPort} onChange={e => setFtpPort(e.target.value)} type="number" min={1} max={65535} className="filter-input" /></div>
        )}
        {plain.length > 0 && (
          <p className="proto-warning" role="note">⚠ {t('remote.proto.plainWarning', { names: plain.join(', ') })}</p>
        )}
        <div className="form-group"><label>{t('remote.user')} *</label>
          <input value={user} onChange={e => setUser(e.target.value)} placeholder="root" className="filter-input" /></div>
        {ssh && (
          <div className="form-group"><label>{t('remote.auth')}</label>
            <select value={auth} onChange={e => setAuth(e.target.value)} className="select-input">
              <option value="key">{t('remote.sshKey')}</option>
              <option value="password">{t('accounts.password')}</option>
            </select></div>
        )}
        {method === 'password' && (
          <div className="form-group"><label>{t('remote.password')}</label>
            <input value={password} onChange={e => setPassword(e.target.value)} type="password" autoComplete="new-password"
              className="filter-input"
              placeholder={server?.has_password ? t('remote.passwordKeep') : t('remote.passwordPlaceholder')} />
            {password !== password.trim() && (
              <small style={{ fontSize: 11, color: 'var(--warning)' }}>{t('remote.passwordWhitespace')}</small>
            )}
            {server?.has_password && (
              <button type="button" className="btn btn-secondary btn-sm" style={{ marginTop: 4 }} onClick={clearPassword}>
                {t('remote.clearPassword')}
              </button>
            )}
            <small className="text-secondary" style={{ fontSize: 11 }}>{t('remote.passwordHint')}</small>
          </div>
        )}
        {files === 'ftps' && (
          <div className="form-group" style={{ gridColumn: '1 / -1' }}><label htmlFor="srv-fingerprint">{t('remote.proto.fingerprint')}</label>
            <input id="srv-fingerprint" value={fingerprint} onChange={e => setFingerprint(e.target.value)} className="filter-input"
              placeholder="SHA256:AB:CD:…" spellCheck={false} />
            <small className="text-secondary" style={{ fontSize: 11 }}>{t('remote.proto.fingerprintHint')}</small></div>
        )}
        <div className="form-group"><label>{t('remote.tags')}</label>
          <input value={tags} onChange={e => setTags(e.target.value)} placeholder="web, production" className="filter-input" /></div>
        <div className="form-group" style={{ gridColumn: '1 / -1' }}><label>{t('remote.memo')}</label>
          <input value={memo} onChange={e => setMemo(e.target.value)} placeholder="..." className="filter-input" /></div>
      </div>
      <div className="btn-group">
        <button type="submit" className="btn btn-primary btn-sm" disabled={saving}>
          {saving ? t('common.working') : server ? t('common.save') : t('remote.addServer')}
        </button>
        {onCancel && <button type="button" className="btn btn-secondary btn-sm" onClick={onCancel}>{t('common.cancel')}</button>}
      </div>
    </form>
  );
}

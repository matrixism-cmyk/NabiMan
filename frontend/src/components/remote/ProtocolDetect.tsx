import React, { useCallback, useEffect, useRef, useState } from 'react';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { ProtocolCheck, RemoteServer } from '../../types';
import { useT } from '../../i18n';
import { describeProtocols } from './ProtocolBadges';
import { DETECT_PENDING_KEY } from './ServerForm';
import { filesOf, protocolName, terminalOf, usesSsh } from './protocols';

interface Detected {
  checks: ProtocolCheck[];
  terminal: string;
  files: string;
  changed: boolean;
}

interface Props {
  server: RemoteServer;
  onChanged: () => void;
  /** Bumped to check again, as when the host key has just been approved. */
  trigger?: number;
}

/** One line per way of reaching the server that was tried. */
export function CheckList({ checks }: { checks: ProtocolCheck[] }) {
  return (
    <ul className="proto-checks">
      {checks.map(c => (
        <li key={c.protocol}>
          <span className={c.ok ? 'is-ok' : 'is-bad'} aria-hidden="true">{c.ok ? '✓' : '✕'}</span>
          <strong>{protocolName(c.protocol)}</strong>
          <small>{c.message}</small>
        </li>
      ))}
    </ul>
  );
}

/**
 * Finds out which protocols the server really offers — a shell and SFTP are
 * tried separately, since SFTP-only accounts and SSH without SFTP are both
 * common — and sets the server to what works. It runs by itself once for a
 * server just added and after its host key is approved; the operator can
 * correct the result in the form below.
 */
export default function ProtocolDetect({ server, onChanged, trigger = 0 }: Props) {
  const { t } = useT();
  const [running, setRunning] = useState(false);
  const [checks, setChecks] = useState<ProtocolCheck[]>([]);
  const [message, setMessage] = useState('');
  // Kept in refs so a re-render (the list polls) never starts another check.
  const latest = useRef(server);
  latest.current = server;
  const changed = useRef(onChanged);
  changed.current = onChanged;

  const run = useCallback(async () => {
    const srv = latest.current;
    setRunning(true);
    setMessage('');
    const res = await apiPost<Detected>(`/api/remote-servers/${srv.id}/detect`, {});
    setRunning(false);
    if (!res.success || !res.data) { setMessage(res.message || t('common.error')); return; }
    const found = res.data;
    setChecks(found.checks);
    const result = describeProtocols(t, [found.terminal, found.files]);
    if (!found.checks.some(c => c.ok)) { setMessage(t('remote.detect.noneWorked')); return; }
    if (!found.changed) { setMessage(t('remote.detect.same', { result })); return; }
    const saved = await apiRequest<RemoteServer>(`/api/remote-servers/${srv.id}`, {
      method: 'PUT', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ terminal: found.terminal, files: found.files }),
    });
    if (!saved.success) { setMessage(saved.message || t('common.error')); return; }
    setMessage(t('remote.detect.applied', { result }));
    changed.current();
  }, [t]);

  // A server just added is looked at once, as soon as it can be: an SSH one
  // only after its host key is approved.
  useEffect(() => {
    let pending = '';
    try { pending = sessionStorage.getItem(DETECT_PENDING_KEY) || ''; } catch { /* private mode */ }
    if (pending !== server.id || (usesSsh(server) && !server.host_trust)) return;
    try { sessionStorage.removeItem(DETECT_PENDING_KEY); } catch { /* private mode */ }
    run();
  }, [server, run]);

  useEffect(() => { if (trigger > 0) run(); }, [trigger, run]);

  return (
    <section className="rw-section" aria-labelledby={`detect-${server.id}`}>
      <div className="rw-section-head">
        <h3 id={`detect-${server.id}`}>{t('remote.detect.title')}</h3>
        <button type="button" className="btn btn-secondary btn-sm" onClick={run} disabled={running}>
          {running ? t('remote.detect.running') : t('remote.detect.run')}
        </button>
      </div>
      <p className="text-secondary">
        {t('remote.detect.hint')} · {describeProtocols(t, [terminalOf(server), filesOf(server)])}
      </p>
      {message && <p role="status">{message}</p>}
      {checks.length > 0 && <CheckList checks={checks} />}
    </section>
  );
}

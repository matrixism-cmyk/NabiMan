import React, { useCallback, useEffect, useState, useId, useRef } from 'react';
import { createPortal } from 'react-dom';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';

export interface ShareLink {
  token: string;
  session_id: string;
  label: string;
  created_unix: number;
  expires_unix: number;
  read_only: boolean;
  last_used_unix: number;
  needs_password: boolean;
  /** Whether the session behind the link is still running. */
  alive?: boolean;
  usable?: boolean;
}

const EXPIRY_CHOICES: { labelKey: string; secs: number }[] = [
  { labelKey: 'share.1h', secs: 3600 },
  { labelKey: 'share.6h', secs: 6 * 3600 },
  { labelKey: 'share.24h', secs: 24 * 3600 },
  { labelKey: 'share.3d', secs: 3 * 86400 },
  { labelKey: 'share.5d', secs: 5 * 86400 },
  { labelKey: 'share.10d', secs: 10 * 86400 },
  { labelKey: 'share.30d', secs: 30 * 86400 },
];

const shareUrl = (token: string) => `${window.location.origin}/share/${token}`;

function formatExpiry(unix: number, t: (k: string) => string): string {
  if (unix === 0) return t('share.forever');
  return new Date(unix * 1000).toLocaleString();
}

interface Props {
  sessionId: string;
  sessionLabel: string;
  onClose: () => void;
}

/** Hand one running terminal to someone who has no NabiMan account. */
export default function ShareDialog({ sessionId, sessionLabel, onClose }: Props) {
  const { t } = useT();
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  const [expiry, setExpiry] = useState(24 * 3600);
  const [password, setPassword] = useState('');
  const [readOnly, setReadOnly] = useState(true);
  const [links, setLinks] = useState<ShareLink[]>([]);
  const [created, setCreated] = useState<ShareLink | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const res = await apiRequest<ShareLink[]>('/api/terminal/share-links');
    if (res.success && res.data) setLinks(res.data.filter((l) => l.session_id === sessionId));
    else setMessage(res.message || t('common.error'));
  }, [sessionId, t]);

  useEffect(() => { load(); }, [load]);

  const create = async () => {
    setBusy(true);
    const res = await apiPost<ShareLink>(`/api/terminal/sessions/${sessionId}/share-link`, {
      expires_in_secs: expiry,
      password: password.trim(),
      read_only: readOnly,
    });
    setBusy(false);
    if (!res.success || !res.data) { setMessage(res.message || t('common.error')); return; }
    setCreated(res.data);
    setPassword('');
    setMessage('');
    load();
  };

  const revoke = async (token: string) => {
    if (!window.confirm(t('share.revokeConfirm'))) return;
    setBusy(true);
    const res = await apiRequest(`/api/terminal/share-links/${token}`, { method: 'DELETE' });
    setBusy(false);
    if (!res.success) { setMessage(res.message || t('common.error')); return; }
    setMessage(t('share.revoked'));
    dialogRef.current?.focus();
    if (created?.token === token) setCreated(null);
    load();
  };

  const copy = (token: string) => {
    navigator.clipboard.writeText(shareUrl(token))
      .then(() => setMessage(t('share.copied')))
      .catch(() => setMessage(shareUrl(token)));
  };

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); closeRef.current(); }
    };
    document.addEventListener('keydown', escape, true);
    return () => { document.removeEventListener('keydown', escape, true); previous?.focus(); };
  }, []);

  return createPortal(
    <div className="modal-overlay term-settings-overlay" onClick={onClose}>
      <div ref={dialogRef} className="modal-content share-dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === 'Tab') {
            const controls = Array.from(e.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), [tabindex="0"]'));
            const first = controls[0], last = controls[controls.length - 1];
            if (e.shiftKey && (document.activeElement === first || document.activeElement === e.currentTarget)) { e.preventDefault(); last?.focus(); }
            else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
          }
        }}>
        <h3 id={titleId}>{t('share.title')}</h3>
        <p className="text-secondary">{sessionLabel}</p>
        <p className="share-warning">{t('share.warning')}</p>
        <p className="text-secondary">{t('share.lifetimePolicy')}</p>
        {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

        <div className="form-group">
          <label>{t('share.expiry')}</label>
          <div className="filter-row">
            {EXPIRY_CHOICES.map((choice) => (
              <button
                key={choice.secs} type="button"
                className={`btn btn-sm ${expiry === choice.secs ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => setExpiry(choice.secs)}
              >{t(choice.labelKey)}</button>
            ))}
          </div>
        </div>

        <div className="form-group">
          <label>{t('share.password')}</label>
          <input
            type="password" className="filter-input" value={password} autoComplete="new-password"
            placeholder={t('share.passwordPlaceholder')}
            onChange={(e) => setPassword(e.target.value)}
          />
          <small className="text-secondary">{t('share.passwordHint')}</small>
        </div>

        <div className="form-group">
          <label>
            <input type="checkbox" checked={readOnly} onChange={(e) => setReadOnly(e.target.checked)} />{' '}
            {t('share.readOnly')}
          </label>
          <small className="text-secondary">{t('share.readOnlyHint')}</small>
        </div>

        <div className="btn-group">
          <button className="btn btn-primary" onClick={create} disabled={busy}>
            {busy ? t('common.working') : t('share.create')}
          </button>
          <button className="btn btn-secondary" onClick={onClose}>{t('common.close')}</button>
        </div>

        {links.length > 0 && (
          <div className="share-list">
            <label>{t('share.existing')}</label>
            <table className="data-table" style={{ fontSize: 12 }}>
              <thead>
                <tr><th>{t('share.link')}</th><th>{t('share.expiresAt')}</th><th>{t('share.options')}</th><th /></tr>
              </thead>
              <tbody>
                {links.map((link) => (
                  <tr key={link.token}>
                    <td>{created?.token === link.token
                      ? <input className="filter-input" aria-label={t('share.linkReady')} readOnly value={shareUrl(link.token)} onFocus={(e) => e.target.select()} />
                      : <code>/share/{link.token.slice(0, 8)}…</code>}</td>
                    <td>{formatExpiry(link.expires_unix, t)}</td>
                    <td>
                      {link.needs_password ? t('share.withPassword') : t('share.noPassword')}
                      {link.read_only ? ` · ${t('share.readOnly')}` : ''}
                      {link.usable === false && <span className="status-badge down">{t('share.unusable')}</span>}
                    </td>
                    <td>
                      <button className="btn btn-secondary btn-sm" style={{ marginRight: 4 }}
                        disabled={link.usable === false} onClick={() => copy(link.token)}>{t('share.copy')}</button>
                      <button className="btn btn-danger btn-sm" disabled={busy} onClick={() => revoke(link.token)}>
                        {t('share.revoke')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>, document.body
  );
}

import React, { useCallback, useEffect, useState, useId } from 'react';
import Modal from '../../shared/ui/Modal';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { useGrants, useSharing } from '../remote/member';

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
  /** File panel access the link grants. Older links have none. */
  files?: ShareFiles;
  /** Set instead of a session for a link to a saved server's files alone. */
  server_id?: string;
}

export type ShareFiles = 'none' | 'read' | 'write';
const FILES_CHOICES: { value: ShareFiles; labelKey: string }[] = [
  { value: 'none', labelKey: 'share.filesNone' },
  { value: 'read', labelKey: 'share.filesRead' },
  { value: 'write', labelKey: 'share.filesWrite' },
];

const EXPIRY_CHOICES: { labelKey: string; secs: number }[] = [
  { labelKey: 'share.1h', secs: 3600 },
  { labelKey: 'share.6h', secs: 6 * 3600 },
  { labelKey: 'share.24h', secs: 24 * 3600 },
  { labelKey: 'share.3d', secs: 3 * 86400 },
  { labelKey: 'share.5d', secs: 5 * 86400 },
  { labelKey: 'share.10d', secs: 10 * 86400 },
  { labelKey: 'share.30d', secs: 30 * 86400 },
  { labelKey: 'share.60d', secs: 60 * 86400 },
  { labelKey: 'share.90d', secs: 90 * 86400 },
  // Lasts until revoked.
  { labelKey: 'share.forever', secs: 0 },
];

/** The end the server gives a permanent link: the last second of year 9999. */
export const PERMANENT_UNIX = 253402300799;
export const isPermanent = (unix: number) => unix >= PERMANENT_UNIX;

const shareUrl = (token: string) => `${window.location.origin}/share/${token}`;

function formatExpiry(unix: number, t: (k: string) => string): string {
  if (unix === 0 || isPermanent(unix)) return t('share.forever');
  return new Date(unix * 1000).toLocaleString();
}

interface Props {
  /** The terminal session to share… */
  sessionId?: string;
  /** …or, for a server reached only for its files, that server's files. */
  serverId?: string;
  sessionLabel: string;
  onClose: () => void;
}

/** Hand one running terminal — or one server's files — to someone who has
 *  no NabiMan account. */
export default function ShareDialog({ sessionId = '', serverId = '', sessionLabel, onClose }: Props) {
  const { t } = useT();
  // A member's session link carries files only when files were granted too.
  const filesGranted = useSharing().files;
  // And keeps to the terms an administrator set for the member's links.
  const grants = useGrants();
  const maxSecs = grants?.share_max_days ? grants.share_max_days * 86400 : 0;
  const expiries = EXPIRY_CHOICES.filter((c) => !maxSecs || (c.secs > 0 && c.secs <= maxSecs));
  const viewOnly = Boolean(grants?.share_view_only);
  const needsPassword = Boolean(grants?.share_require_password);
  const titleId = useId();
  const [expiry, setExpiry] = useState(maxSecs && maxSecs < 24 * 3600 ? expiries[expiries.length - 1].secs : 24 * 3600);
  const [password, setPassword] = useState('');
  const [readOnly, setReadOnly] = useState(true);
  // A files link is nothing without files: it starts on browsing.
  const filesOnly = Boolean(serverId);
  const [files, setFiles] = useState<ShareFiles>(filesOnly ? 'read' : 'none');
  const [links, setLinks] = useState<ShareLink[]>([]);
  const [created, setCreated] = useState<ShareLink | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const res = await apiRequest<ShareLink[]>('/api/terminal/share-links');
    if (res.success && res.data) {
      setLinks(res.data.filter((l) => (filesOnly ? l.server_id === serverId : l.session_id === sessionId)));
    } else setMessage(res.message || t('common.error'));
  }, [sessionId, serverId, filesOnly, t]);

  useEffect(() => { load(); }, [load]);

  const create = async () => {
    if (needsPassword && !password.trim()) { setMessage(t('share.passwordRequired')); return; }
    setBusy(true);
    const res = await apiPost<ShareLink>(filesOnly
      ? `/api/terminal/files/${serverId}/share-link`
      : `/api/terminal/sessions/${sessionId}/share-link`, {
      expires_in_secs: expiry,
      permanent: expiry === 0,
      password: password.trim(),
      read_only: readOnly || viewOnly,
      files,
    });
    setBusy(false);
    if (!res.success || !res.data) { setMessage(res.message || t('common.error')); return; }
    setCreated(res.data);
    setPassword('');
    setMessage('');
    load();
  };

  const revoke = async (token: string) => {
    if (!(await confirmDialog({ message: t('share.revokeConfirm'), danger: true, confirmLabel: t('common.delete') }))) return;
    setBusy(true);
    const res = await apiRequest(`/api/terminal/share-links/${token}`, { method: 'DELETE' });
    setBusy(false);
    if (!res.success) { setMessage(res.message || t('common.error')); return; }
    setMessage(t('share.revoked'));
    if (created?.token === token) setCreated(null);
    load();
  };

  const copy = (token: string) => {
    navigator.clipboard.writeText(shareUrl(token))
      .then(() => setMessage(t('share.copied')))
      .catch(() => setMessage(shareUrl(token)));
  };

  return (
    <Modal className="share-dialog" onClose={onClose} title={t(filesOnly ? 'share.filesTitle' : 'share.title')}>
        <p className="text-secondary">{sessionLabel}</p>
        <p className="share-warning">{t(filesOnly ? 'share.filesWarning' : 'share.warning')}</p>
        <p className="text-secondary">{t('share.lifetimePolicy')}</p>
        {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

        <div className="form-group">
          <label>{t('share.expiry')}</label>
          <div className="filter-row">
            {expiries.map((choice) => (
              <button
                key={choice.secs} type="button"
                className={`btn btn-sm ${expiry === choice.secs ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => setExpiry(choice.secs)}
              >{t(choice.labelKey)}</button>
            ))}
          </div>
          {expiry === 0 && <small className="share-warning">{t('share.foreverHint')}</small>}
        </div>

        <div className="form-group">
          <label>{t('share.password')}</label>
          <input
            type="password" className="filter-input" value={password} autoComplete="new-password"
            placeholder={t('share.passwordPlaceholder')}
            onChange={(e) => setPassword(e.target.value)}
          />
          <small className="text-secondary">{t(needsPassword ? 'share.passwordRequired' : 'share.passwordHint')}</small>
        </div>

        {!filesOnly && (
          <div className="form-group">
            <label>
              <input type="checkbox" checked={readOnly || viewOnly} disabled={viewOnly} onChange={(e) => setReadOnly(e.target.checked)} />{' '}
              {t('share.readOnly')}
            </label>
            <small className="text-secondary">{t('share.readOnlyHint')}</small>
          </div>
        )}

        {filesGranted && <div className="form-group">
          <label id={`${titleId}-files`}>{t('share.files')}</label>
          <div className="filter-row" role="radiogroup" aria-labelledby={`${titleId}-files`}>
            {FILES_CHOICES.filter((choice) => (!filesOnly || choice.value !== 'none') && !(viewOnly && choice.value === 'write')).map((choice) => (
              <button
                key={choice.value} type="button" role="radio" aria-checked={files === choice.value}
                className={`btn btn-sm ${files === choice.value ? 'btn-primary' : 'btn-secondary'}`}
                onClick={() => setFiles(choice.value)}
              >{t(choice.labelKey)}</button>
            ))}
          </div>
          <small className="text-secondary">{t('share.filesHint')}</small>
        </div>}

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
                      {link.read_only && !link.server_id ? ` · ${t('share.readOnly')}` : ''}
                      {link.files === 'read' ? ` · ${t('share.filesTagRead')}` : link.files === 'write' ? ` · ${t('share.filesTagWrite')}` : ''}
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
    </Modal>
  );
}

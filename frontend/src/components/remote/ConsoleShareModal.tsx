import React, { useEffect, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { ConsoleShare, createShare, listShares, revokeShare, shareLink } from './vncApi';

// Mirrors the terminal share's expiry choices (in seconds; 0 = until revoked).
const EXPIRY: { key: string; secs: number }[] = [
  { key: 'share.1h', secs: 3600 },
  { key: 'share.6h', secs: 6 * 3600 },
  { key: 'share.24h', secs: 24 * 3600 },
  { key: 'share.3d', secs: 3 * 86400 },
  { key: 'share.5d', secs: 5 * 86400 },
  { key: 'share.10d', secs: 10 * 86400 },
  { key: 'share.30d', secs: 30 * 86400 },
  { key: 'share.60d', secs: 60 * 86400 },
  { key: 'share.90d', secs: 90 * 86400 },
  { key: 'share.forever', secs: 0 },
];
const PERMANENT = 253_402_300_799;

/** Make and manage console share links — the terminal share dialog, for a VM
 *  console: an expiry preset, an optional password, and read-only vs control. */
export default function ConsoleShareModal({ console: key, label, onClose }: {
  console: string; label: string; onClose: () => void;
}) {
  const { t } = useT();
  const [secs, setSecs] = useState(24 * 3600);
  const [readOnly, setReadOnly] = useState(true);
  const [password, setPassword] = useState('');
  const [shares, setShares] = useState<ConsoleShare[]>([]);
  const [issued, setIssued] = useState('');
  const [copied, setCopied] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');

  const reload = () => { listShares(key).then((r) => { if (r.success && r.data) setShares(r.data); }); };
  useEffect(reload, [key]);

  const make = async () => {
    setBusy(true); setMessage('');
    const r = await createShare(key, readOnly ? 'view' : 'control', Math.round(secs / 3600), password.trim() || undefined);
    setBusy(false);
    if (r.success && r.data) { setIssued(shareLink(r.data.token)); setPassword(''); reload(); }
    else setMessage(r.message || t('common.error'));
  };
  const copy = (url: string, tag: string) => { navigator.clipboard.writeText(url).then(() => { setCopied(tag); setTimeout(() => setCopied(''), 1500); }).catch(() => setMessage(url)); };
  const drop = async (token: string) => {
    if (!(await confirmDialog({ message: t('remote.console.share.revokeConfirm'), danger: true, confirmLabel: t('remote.console.share.revoke') }))) return;
    await revokeShare(key, token); if (issued.endsWith(token)) setIssued(''); reload();
  };
  const when = (ms: number) => (ms >= PERMANENT ? t('share.forever') : new Date(ms * 1000).toLocaleString());

  return (
    <Modal className="share-dialog console-share" onClose={onClose} title={`${t('remote.console.share.title')} · ${label}`}>
      <p className="share-warning">{t('remote.console.share.warning')}</p>
      <p className="text-secondary">{t('share.lifetimePolicy')}</p>
      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      <div className="form-group">
        <label>{t('share.expiry')}</label>
        <div className="filter-row">
          {EXPIRY.map((o) => (
            <button key={o.secs} type="button" className={`btn btn-sm ${secs === o.secs ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setSecs(o.secs)}>{t(o.key)}</button>
          ))}
        </div>
        {secs === 0 && <small className="share-warning">{t('share.foreverHint')}</small>}
      </div>

      <div className="form-group">
        <label>{t('share.password')}</label>
        <input type="password" className="filter-input" value={password} autoComplete="new-password"
          placeholder={t('share.passwordPlaceholder')} onChange={(e) => setPassword(e.target.value)} />
        <small className="text-secondary">{t('share.passwordHint')}</small>
      </div>

      <div className="form-group">
        <label><input type="checkbox" checked={readOnly} onChange={(e) => setReadOnly(e.target.checked)} /> {t('share.readOnly')}</label>
        <small className="text-secondary">{t('share.readOnlyHint')}</small>
      </div>

      <div className="btn-group">
        <button className="btn btn-primary" onClick={make} disabled={busy}>{busy ? t('common.working') : t('share.create')}</button>
        <button className="btn btn-secondary" onClick={onClose}>{t('common.close')}</button>
      </div>

      {issued && (
        <div className="console-share-issued" style={{ margin: '10px 0' }}>
          <div style={{ display: 'flex', gap: 8 }}>
            <input readOnly className="filter-input" value={issued} style={{ flex: 1 }} onFocus={(e) => e.target.select()} />
            <button className="btn btn-secondary btn-sm" onClick={() => copy(issued, 'new')}>{copied === 'new' ? t('share.copied') : t('common.copy')}</button>
          </div>
        </div>
      )}

      {shares.length > 0 && (
        <div className="share-list">
          <h4>{t('remote.console.share.active')} <em>{shares.length}</em></h4>
          <table className="data-table" style={{ fontSize: 13 }}>
            <thead><tr><th>{t('remote.console.share.link')}</th><th>{t('share.readOnly')}</th><th>{t('remote.console.share.expires')}</th><th /></tr></thead>
            <tbody>{shares.map((s) => (
              <tr key={s.token}>
                <td><button className="btn btn-secondary btn-sm" onClick={() => copy(shareLink(s.token), s.token)}>{copied === s.token ? t('share.copied') : t('common.copy')}</button></td>
                <td>{s.mode === 'control' ? t('remote.console.share.control') : t('share.readOnly')}{s.needs_password ? ' 🔒' : ''}</td>
                <td>{when(s.expires_unix)}</td>
                <td><button className="btn btn-danger btn-sm" onClick={() => drop(s.token)}>{t('remote.console.share.revoke')}</button></td>
              </tr>
            ))}</tbody>
          </table>
        </div>
      )}
    </Modal>
  );
}

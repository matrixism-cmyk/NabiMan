import React, { useEffect, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { ConsoleShare, createShare, listShares, revokeShare, shareLink } from './vncApi';

/** Make and manage share links for a console — like a terminal share link, but
 *  per share you choose view-only or control. Anyone who can open the console
 *  can make one. */
export default function ConsoleShareModal({ console: key, label, onClose }: {
  console: string; label: string; onClose: () => void;
}) {
  const { t } = useT();
  const [mode, setMode] = useState<'view' | 'control'>('view');
  const [hours, setHours] = useState(24);
  const [shares, setShares] = useState<ConsoleShare[]>([]);
  const [issued, setIssued] = useState<string>('');
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');

  const reload = () => { listShares(key).then((r) => { if (r.success && r.data) setShares(r.data); }); };
  useEffect(reload, [key]);

  const make = async () => {
    setBusy(true); setMessage('');
    const r = await createShare(key, mode, hours);
    setBusy(false);
    if (r.success && r.data) { setIssued(shareLink(r.data.token)); reload(); }
    else setMessage(r.message || t('common.error'));
  };
  const copy = () => { navigator.clipboard.writeText(issued).then(() => { setCopied(true); setTimeout(() => setCopied(false), 1500); }).catch(() => { /* ignore */ }); };
  const drop = async (token: string) => {
    if (!(await confirmDialog({ message: t('remote.console.share.revokeConfirm'), danger: true, confirmLabel: t('remote.console.share.revoke') }))) return;
    await revokeShare(key, token); if (issued.endsWith(token)) setIssued(''); reload();
  };
  const when = (ms: number) => new Date(ms * 1000).toLocaleString();

  return (
    <Modal className="console-share" onClose={onClose} title={`${t('remote.console.share.title')} · ${label}`}>
      <p className="text-secondary" style={{ fontSize: 13 }}>{t('remote.console.share.intro')}</p>
      <div className="console-share-make" style={{ display: 'flex', gap: 8, alignItems: 'flex-end', flexWrap: 'wrap' }}>
        <label>{t('remote.console.share.mode')}
          <select className="select-input" value={mode} onChange={(e) => setMode(e.target.value as 'view' | 'control')}>
            <option value="view">{t('remote.console.share.view')}</option>
            <option value="control">{t('remote.console.share.control')}</option>
          </select>
        </label>
        <label>{t('remote.console.share.hours')}
          <input className="filter-input" type="number" min={1} max={168} value={hours} onChange={(e) => setHours(Math.max(1, Math.min(168, Number(e.target.value) || 1)))} style={{ width: 90 }} />
        </label>
        <button className="btn btn-primary btn-sm" onClick={make} disabled={busy}>{t('remote.console.share.create')}</button>
      </div>
      {issued && (
        <div className="console-share-issued" style={{ margin: '10px 0' }}>
          <p className="text-secondary" style={{ fontSize: 12 }}>{t('remote.console.share.issued')}</p>
          <div style={{ display: 'flex', gap: 8 }}>
            <input readOnly className="filter-input" value={issued} style={{ flex: 1 }} onFocus={(e) => e.target.select()} />
            <button className="btn btn-secondary btn-sm" onClick={copy}>{copied ? t('sshKey.copied') : t('common.copy')}</button>
          </div>
        </div>
      )}
      {message && <p role="alert" className="message">{message}</p>}
      <h4 style={{ marginTop: 12 }}>{t('remote.console.share.active')} <em>{shares.length}</em></h4>
      {shares.length === 0 ? <p className="text-secondary">{t('remote.console.share.none')}</p> : (
        <table className="data-table" style={{ fontSize: 13 }}>
          <thead><tr><th>{t('remote.console.share.mode')}</th><th>{t('remote.console.share.expires')}</th><th /></tr></thead>
          <tbody>{shares.map((s) => (
            <tr key={s.token}>
              <td>{s.mode === 'control' ? t('remote.console.share.control') : t('remote.console.share.view')}</td>
              <td>{when(s.expires_unix)}</td>
              <td><button className="btn btn-danger btn-sm" onClick={() => drop(s.token)}>{t('remote.console.share.revoke')}</button></td>
            </tr>
          ))}</tbody>
        </table>
      )}
    </Modal>
  );
}

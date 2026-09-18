import React, { useCallback, useEffect, useState } from 'react';
import { apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';
import { ShareLink } from './ShareDialog';

const shareUrl = (token: string) => `${window.location.origin}/share/${token}`;

/** "in 3 days", "in 20 minutes" — how long this link has left. */
function remaining(expires: number, t: (k: string) => string): string {
  if (expires === 0) return t('share.forever');
  const secs = expires - Math.floor(Date.now() / 1000);
  if (secs <= 0) return t('share.expiredNow');
  // Round up: a link made for 3 days should read "3일", not "2일".
  const days = Math.ceil(secs / 86400);
  if (secs >= 86400) return t('share.inDays').replace('{n}', String(days));
  const hours = Math.ceil(secs / 3600);
  if (secs >= 3600) return t('share.inHours').replace('{n}', String(hours));
  return t('share.inMinutes').replace('{n}', String(Math.max(1, Math.floor(secs / 60))));
}

function used(unix: number, t: (k: string) => string): string {
  return unix === 0 ? t('share.neverUsed') : new Date(unix * 1000).toLocaleString();
}

/** Every link this account has handed out, wherever it was created. */
export default function ShareLinksPanel() {
  const { t } = useT();
  const [links, setLinks] = useState<ShareLink[]>([]);
  const [message, setMessage] = useState('');
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const res = await apiRequest<ShareLink[]>('/api/terminal/share-links');
    if (res.success && res.data) setLinks(res.data);
    setLoading(false);
  }, []);

  useEffect(() => {
    load();
    const id = window.setInterval(load, 30000);
    return () => window.clearInterval(id);
  }, [load]);

  const revoke = async (link: ShareLink) => {
    if (!window.confirm(t('share.revokeConfirm'))) return;
    await apiRequest(`/api/terminal/share-links/${link.token}`, { method: 'DELETE' });
    setMessage(t('share.revoked'));
    load();
  };

  const copy = (token: string) => {
    navigator.clipboard.writeText(shareUrl(token))
      .then(() => setMessage(t('share.copied')))
      .catch(() => setMessage(shareUrl(token)));
  };

  if (loading) return <div className="rw-section"><p className="text-secondary">{t('common.loading')}</p></div>;

  return (
    <div className="rw-section">
      <div className="rw-section-head">
        <h3>{t('share.panelTitle')}</h3>
        <button className="btn btn-secondary btn-sm" onClick={load}>{t('common.refresh')}</button>
      </div>
      {message && <div className="message" onClick={() => setMessage('')}>{message}</div>}

      {links.length === 0 ? (
        <p className="text-secondary">{t('share.none')}</p>
      ) : (
        <>
          <p className="share-warning">{t('share.panelWarning')}</p>
          <div style={{ overflowX: 'auto' }}>
            <table className="data-table" style={{ fontSize: 12 }}>
              <thead>
                <tr>
                  <th>{t('terminal.label')}</th>
                  <th>{t('share.link')}</th>
                  <th>{t('share.expiresIn')}</th>
                  <th>{t('share.options')}</th>
                  <th>{t('share.lastUsed')}</th>
                  <th>{t('common.actions')}</th>
                </tr>
              </thead>
              <tbody>
                {links.map((link) => (
                  <tr key={link.token}>
                    <td>
                      <strong>{link.label}</strong>
                      {!link.alive && <span className="status-badge down" style={{ marginLeft: 6 }}>{t('share.ended')}</span>}
                    </td>
                    <td><code>/share/{link.token.slice(0, 8)}…</code></td>
                    <td>{remaining(link.expires_unix, t)}</td>
                    <td>
                      {link.needs_password ? t('share.withPassword') : t('share.noPassword')}
                      {link.read_only ? ` · ${t('share.readOnly')}` : ''}
                    </td>
                    <td>{used(link.last_used_unix, t)}</td>
                    <td>
                      <button className="btn btn-secondary btn-sm" style={{ marginRight: 4 }}
                        onClick={() => copy(link.token)}>{t('share.copy')}</button>
                      <button className="btn btn-danger btn-sm" onClick={() => revoke(link)}>{t('share.revoke')}</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}

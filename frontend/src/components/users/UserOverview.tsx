import React from 'react';
import { apiPost, useApi } from '../../shared/api/useApi';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { useT } from '../../i18n';
import type { MemberUser } from '../MembershipEditor';
import { when } from './userModel';

interface ServerLine { id: string; name: string; address: string; status: string }
interface Overview {
  own_servers: ServerLine[];
  assigned_servers: ServerLine[];
  terminal_sessions: { id: string; label: string; title: string; created_unix: number; last_active_unix: number; keepalive: boolean; viewers: number }[];
  share_links: { token: string; label: string; expires_unix: number; files: string; read_only: boolean; usable: boolean; needs_password: boolean }[];
  sign_ins: { session_id: string; created_at: string; last_active: string; ip: string }[];
}

const PERMANENT = 253402300799;

/** What the account has right now: servers, terminals, share links and sign-ins. */
export default function UserOverview({ user, onMessage }: { user: MemberUser; onMessage: (m: string) => void }) {
  const { data, loading, error, refetch } = useApi<Overview>(`/api/users/${user.id}/overview`, 15000);
  const { t } = useT();
  if (loading && !data) return <p className="text-secondary">{t('users.overview.loading')}</p>;
  if (error || !data) return <p role="alert" className="message">{error || t('users.overview.failed')}</p>;

  const revokeLinks = async () => {
    if (!(await confirmDialog({ message: t('users.overview.revokeConfirm', { name: user.username, n: data.share_links.length }), danger: true, confirmLabel: t('users.overview.revokeAll') }))) return;
    const res = await apiPost<string>(`/api/users/${user.id}/share-links/revoke`, {});
    onMessage(res.data || res.message || '');
    refetch();
  };
  const servers = (title: string, list: ServerLine[], empty: string) => (
    <section className="users-card">
      <h4>{title} <em>{list.length}</em></h4>
      {list.length === 0 ? <p className="text-secondary">{empty}</p> : (
        <ul className="users-lines">
          {list.map(s => (
            <li key={s.id}>
              <span className={`users-dot is-${s.status === 'online' ? 'ok' : s.status === 'offline' ? 'bad' : 'muted'}`} />
              <strong>{s.name}</strong><code>{s.address}</code>
            </li>
          ))}
        </ul>
      )}
    </section>
  );

  return (
    <div className="users-overview">
      <div className="users-numbers">
        <div><strong>{data.terminal_sessions.length}</strong><span>{t('users.overview.terminals')}</span></div>
        <div><strong>{data.share_links.filter(l => l.usable).length}</strong><span>{t('users.overview.validLinks')}</span></div>
        <div><strong>{data.sign_ins.length}</strong><span>{t('users.overview.signedIn')}</span></div>
        <div><strong>{data.own_servers.length + data.assigned_servers.length}</strong><span>{t('users.overview.usableServers')}</span></div>
      </div>

      {user.role === 'remote' && servers(t('users.overview.ownServers'), data.own_servers, t('users.overview.noOwnServers'))}
      {user.role !== 'admin' && servers(user.role === 'remote' ? t('users.overview.assignedServers') : t('users.overview.scopeServers'),
        data.assigned_servers, t('users.overview.noAssigned'))}

      <section className="users-card">
        <h4>{t('users.overview.terminals')} <em>{data.terminal_sessions.length}</em></h4>
        {data.terminal_sessions.length === 0 ? <p className="text-secondary">{t('users.overview.noSessions')}</p> : (
          <ul className="users-lines">
            {data.terminal_sessions.map(s => (
              <li key={s.id}>
                <strong>{s.title || s.label}</strong>
                <span className="text-secondary">{t('users.overview.lastUsed', { when: when(s.last_active_unix) })}{s.viewers ? t('users.overview.watching', { n: s.viewers }) : ''}{s.keepalive ? t('users.overview.kept') : ''}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="users-card">
        <h4>{t('users.overview.links')} <em>{data.share_links.length}</em>
          {data.share_links.length > 0 && <button type="button" className="btn btn-secondary btn-sm" onClick={revokeLinks}>{t('users.overview.revokeAll')}</button>}
        </h4>
        {data.share_links.length === 0 ? <p className="text-secondary">{t('users.overview.noLinks')}</p> : (
          <ul className="users-lines">
            {data.share_links.map(l => (
              <li key={l.token}>
                <span className={`users-dot is-${l.usable ? 'ok' : 'muted'}`} />
                <strong>{l.label}</strong>
                <span className="text-secondary">
                  {l.usable ? '' : t('users.overview.unusable')}
                  {l.expires_unix >= PERMANENT ? t('users.overview.permanent') : `~${when(l.expires_unix)}`}
                  {l.read_only ? t('users.overview.viewOnly') : ''}{l.files !== 'none' ? t(l.files === 'write' ? 'users.overview.filesWrite' : 'users.overview.filesRead') : ''}
                  {l.needs_password ? t('users.overview.withPassword') : ''}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="users-card">
        <h4>{t('users.overview.signIns')} <em>{data.sign_ins.length}</em></h4>
        {data.sign_ins.length === 0 ? <p className="text-secondary">{t('users.overview.noSignIns')}</p> : (
          <ul className="users-lines">
            {data.sign_ins.map(s => (
              <li key={s.session_id}>
                <code>{s.ip || t('users.overview.unknownIp')}</code>
                <span className="text-secondary">{t('users.overview.session', { start: when(s.created_at), last: when(s.last_active) })}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

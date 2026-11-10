import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import { useT } from '../../i18n';
import type { MemberUser, ScopeOption } from '../MembershipEditor';
import UserOverview from './UserOverview';
import UserAccount from './UserAccount';
import UserScopes from './UserScopes';
import RemoteGrantsForm from './RemoteGrantsForm';
import AccessRulesForm from './AccessRulesForm';
import { expired, ROLE_HINT, roleName, STATUS_LABEL, STATUS_TONE, when } from './userModel';

type Tab = 'overview' | 'account' | 'scopes' | 'remote' | 'access';

interface Props {
  user: MemberUser;
  me: MemberUser;
  options: ScopeOption[];
  onChanged: () => void;
  onMessage: (message: string) => void;
  onClose: () => void;
  onDeleted: () => void;
}

/** One member, everything about them: a header with what can be done at once, and a tab per subject. */
export default function UserDetail({ user, me, options, onChanged, onMessage, onClose, onDeleted }: Props) {
  const [tab, setTab] = useState<Tab>(user.status === 'pending' ? 'scopes' : 'overview');
  const [busy, setBusy] = useState(false);
  const { t } = useT();
  const self = user.id === me.id;
  const admin = user.role === 'admin';
  const remote = user.role === 'remote';

  /** Runs one change and reports it; the list reloads when it worked. */
  const act = async (run: () => Promise<{ success: boolean; data?: unknown; message?: string }>) => {
    setBusy(true);
    try {
      const res = await run();
      onMessage(typeof res.data === 'string' && res.data ? res.data : res.message || (res.success ? t('users.detail.saved') : t('users.detail.failed')));
      if (res.success) onChanged();
      return res.success;
    } catch {
      onMessage(t('users.requestFailed'));
      return false;
    } finally {
      setBusy(false);
    }
  };
  const review = (status: MemberUser['status'], memberships = user.memberships) =>
    act(() => apiPost<string>(`/api/users/${user.id}/review`, { status, memberships }));
  const update = (body: object) =>
    act(() => apiRequest<string>(`/api/users/${user.id}`, { method: 'PUT', body: JSON.stringify(body) }));

  const signOut = async () => {
    if (await confirmDialog({ message: t('users.detail.signOutConfirm', { name: user.username }), danger: true, confirmLabel: t('users.detail.signOut') })) {
      act(() => apiPost<string>(`/api/users/${user.id}/sign-out`, {}));
    }
  };
  const remove = async () => {
    if (await confirmDialog({ message: t('users.detail.deleteConfirm', { name: user.username }), danger: true, confirmLabel: t('common.delete') })) {
      if (await act(() => apiRequest<string>(`/api/users/${user.id}`, { method: 'DELETE' }))) onDeleted();
    }
  };

  const tabs: [Tab, string][] = [
    ['overview', t('users.detail.tabOverview')],
    ['account', t('users.detail.tabAccount')],
    ['scopes', remote ? t('users.detail.tabAssigned') : t('users.detail.tabScopes')],
    ...(remote ? [['remote', t('users.detail.tabRemote')] as [Tab, string]] : []),
    ...(!admin ? [['access', t('users.detail.tabAccess')] as [Tab, string]] : []),
  ];

  return (
    <aside className="users-detail" aria-label={t('users.detail.label', { name: user.username })}>
      <header className="users-detail-head">
        <div className="users-detail-title">
          <h3>{user.username}{self && <span className="users-me">{t('users.me')}</span>}</h3>
          <button type="button" className="users-close" onClick={onClose} aria-label={t('users.detail.close')}>×</button>
        </div>
        <div className="users-detail-badges">
          <span className={`users-role role-${user.role}`}>{roleName(t, user.role)}</span>
          <span className={`users-state ${STATUS_TONE[user.status]}`}>{t(STATUS_LABEL[user.status])}</span>
          {expired(user) && <span className="users-state is-bad">{t('users.detail.expired')}</span>}
          <span className={user.totp_enabled ? 'users-chip is-ok' : 'users-chip'}>{user.totp_enabled ? t('users.detail.mfaOn') : t('users.detail.mfaOff')}</span>
        </div>
        <p className="text-secondary users-detail-sub">
          {t('users.detail.sub', { hint: ROLE_HINT[user.role] ? t(ROLE_HINT[user.role]) : '', created: when(user.created_at), login: when(user.last_login) })}
        </p>
        <div className="btn-group users-actions">
          {user.status === 'pending' && <>
            <button type="button" className="btn btn-primary btn-sm" disabled={busy}
              onClick={() => review('active', user.requested_memberships)}>{t('users.detail.approveRequested')}</button>
            <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => review('rejected', [])}>{t('users.detail.reject')}</button>
          </>}
          {user.status === 'active' && !self && (
            <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => review('suspended')}>{t('users.detail.suspend')}</button>
          )}
          {(user.status === 'suspended' || user.status === 'rejected') && (
            <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => review('active')}>
              {user.status === 'suspended' ? t('users.detail.unsuspend') : t('users.detail.reapprove')}
            </button>
          )}
          {!self && <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={signOut}>{t('users.detail.signOut')}</button>}
          {!self && <button type="button" className="btn btn-danger btn-sm" disabled={busy} onClick={remove}>{t('common.delete')}</button>}
        </div>
      </header>

      <nav className="users-tabs" role="tablist" aria-label={t('users.detail.tabs')}>
        {tabs.map(([key, text]) => (
          <button key={key} type="button" role="tab" aria-selected={tab === key}
            className={tab === key ? 'is-on' : ''} onClick={() => setTab(key)}>{text}</button>
        ))}
      </nav>

      <div className="users-tab-body">
        {tab === 'overview' && <UserOverview user={user} onMessage={onMessage} />}
        {tab === 'account' && <UserAccount user={user} self={self} busy={busy} update={update} />}
        {tab === 'scopes' && <UserScopes user={user} options={options} busy={busy} review={review} />}
        {tab === 'remote' && remote && <RemoteGrantsForm user={user} busy={busy} update={update} />}
        {tab === 'access' && !admin && <AccessRulesForm user={user} busy={busy} update={update} />}
      </div>
    </aside>
  );
}

import React, { useState } from 'react';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import type { MemberUser, ScopeOption } from '../MembershipEditor';
import UserOverview from './UserOverview';
import UserAccount from './UserAccount';
import UserScopes from './UserScopes';
import RemoteGrantsForm from './RemoteGrantsForm';
import AccessRulesForm from './AccessRulesForm';
import { expired, ROLE_HINT, ROLE_LABEL, STATUS_LABEL, STATUS_TONE, when } from './userModel';

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
  const self = user.id === me.id;
  const admin = user.role === 'admin';
  const remote = user.role === 'remote';

  /** Runs one change and reports it; the list reloads when it worked. */
  const act = async (run: () => Promise<{ success: boolean; data?: unknown; message?: string }>) => {
    setBusy(true);
    try {
      const res = await run();
      onMessage(typeof res.data === 'string' && res.data ? res.data : res.message || (res.success ? '저장했습니다' : '처리하지 못했습니다'));
      if (res.success) onChanged();
      return res.success;
    } catch {
      onMessage('요청을 처리할 수 없습니다. 다시 시도하세요.');
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
    if (await confirmDialog({ message: `${user.username} 의 모든 로그인을 끝낼까요? 열려 있는 터미널도 끊깁니다.`, danger: true, confirmLabel: '강제 로그아웃' })) {
      act(() => apiPost<string>(`/api/users/${user.id}/sign-out`, {}));
    }
  };
  const remove = async () => {
    if (await confirmDialog({ message: `${user.username} 회원을 삭제할까요? 되돌릴 수 없습니다.`, danger: true, confirmLabel: '삭제' })) {
      if (await act(() => apiRequest<string>(`/api/users/${user.id}`, { method: 'DELETE' }))) onDeleted();
    }
  };

  const tabs: [Tab, string][] = [
    ['overview', '개요'],
    ['account', '계정'],
    ['scopes', remote ? '지정 서버' : '서버·소속'],
    ...(remote ? [['remote', '원격 권한'] as [Tab, string]] : []),
    ...(!admin ? [['access', '접속 제한'] as [Tab, string]] : []),
  ];

  return (
    <aside className="users-detail" aria-label={`${user.username} 상세`}>
      <header className="users-detail-head">
        <div className="users-detail-title">
          <h3>{user.username}{self && <span className="users-me">나</span>}</h3>
          <button type="button" className="users-close" onClick={onClose} aria-label="상세 닫기">×</button>
        </div>
        <div className="users-detail-badges">
          <span className={`users-role role-${user.role}`}>{ROLE_LABEL[user.role] || user.role}</span>
          <span className={`users-state ${STATUS_TONE[user.status]}`}>{STATUS_LABEL[user.status]}</span>
          {expired(user) && <span className="users-state is-bad">이용 기간 만료</span>}
          <span className={user.totp_enabled ? 'users-chip is-ok' : 'users-chip'}>{user.totp_enabled ? '2단계 인증 사용' : '2단계 인증 없음'}</span>
        </div>
        <p className="text-secondary users-detail-sub">
          {ROLE_HINT[user.role] || ''} 가입 {when(user.created_at)} · 최근 로그인 {when(user.last_login)}
        </p>
        <div className="btn-group users-actions">
          {user.status === 'pending' && <>
            <button type="button" className="btn btn-primary btn-sm" disabled={busy}
              onClick={() => review('active', user.requested_memberships)}>신청대로 승인</button>
            <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => review('rejected', [])}>반려</button>
          </>}
          {user.status === 'active' && !self && (
            <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => review('suspended')}>이용 정지</button>
          )}
          {(user.status === 'suspended' || user.status === 'rejected') && (
            <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => review('active')}>
              {user.status === 'suspended' ? '정지 해제' : '다시 승인'}
            </button>
          )}
          {!self && <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={signOut}>강제 로그아웃</button>}
          {!self && <button type="button" className="btn btn-danger btn-sm" disabled={busy} onClick={remove}>삭제</button>}
        </div>
      </header>

      <nav className="users-tabs" role="tablist" aria-label="회원 상세">
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

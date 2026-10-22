import React from 'react';
import { apiPost, useApi } from '../../shared/api/useApi';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
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
  if (loading && !data) return <p className="text-secondary">현황을 불러오는 중…</p>;
  if (error || !data) return <p role="alert" className="message">{error || '현황을 불러오지 못했습니다'}</p>;

  const revokeLinks = async () => {
    if (!(await confirmDialog({ message: `${user.username} 의 공유 링크 ${data.share_links.length}개를 모두 해제할까요?`, danger: true, confirmLabel: '모두 해제' }))) return;
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
        <div><strong>{data.terminal_sessions.length}</strong><span>터미널 세션</span></div>
        <div><strong>{data.share_links.filter(l => l.usable).length}</strong><span>유효한 공유 링크</span></div>
        <div><strong>{data.sign_ins.length}</strong><span>로그인 중</span></div>
        <div><strong>{data.own_servers.length + data.assigned_servers.length}</strong><span>쓸 수 있는 서버</span></div>
      </div>

      {user.role === 'remote' && servers('직접 등록한 서버', data.own_servers, '직접 등록한 서버가 없습니다.')}
      {user.role !== 'admin' && servers(user.role === 'remote' ? '관리자가 지정한 서버' : '소속 서버',
        data.assigned_servers, '지정된 서버가 없습니다.')}

      <section className="users-card">
        <h4>터미널 세션 <em>{data.terminal_sessions.length}</em></h4>
        {data.terminal_sessions.length === 0 ? <p className="text-secondary">열린 세션이 없습니다.</p> : (
          <ul className="users-lines">
            {data.terminal_sessions.map(s => (
              <li key={s.id}>
                <strong>{s.title || s.label}</strong>
                <span className="text-secondary">마지막 사용 {when(s.last_active_unix)}{s.viewers ? ` · 보는 중 ${s.viewers}` : ''}{s.keepalive ? ' · 세션 유지' : ''}</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="users-card">
        <h4>공유 링크 <em>{data.share_links.length}</em>
          {data.share_links.length > 0 && <button type="button" className="btn btn-secondary btn-sm" onClick={revokeLinks}>모두 해제</button>}
        </h4>
        {data.share_links.length === 0 ? <p className="text-secondary">만든 공유 링크가 없습니다.</p> : (
          <ul className="users-lines">
            {data.share_links.map(l => (
              <li key={l.token}>
                <span className={`users-dot is-${l.usable ? 'ok' : 'muted'}`} />
                <strong>{l.label}</strong>
                <span className="text-secondary">
                  {l.usable ? '' : '사용 불가 · '}
                  {l.expires_unix >= PERMANENT ? '영구' : `~${when(l.expires_unix)}`}
                  {l.read_only ? ' · 보기 전용' : ''}{l.files !== 'none' ? ` · 파일 ${l.files === 'write' ? '쓰기' : '읽기'}` : ''}
                  {l.needs_password ? ' · 비밀번호' : ''}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="users-card">
        <h4>로그인 <em>{data.sign_ins.length}</em></h4>
        {data.sign_ins.length === 0 ? <p className="text-secondary">지금 로그인한 곳이 없습니다.</p> : (
          <ul className="users-lines">
            {data.sign_ins.map(s => (
              <li key={s.session_id}>
                <code>{s.ip || '주소 모름'}</code>
                <span className="text-secondary">시작 {when(s.created_at)} · 마지막 활동 {when(s.last_active)}</span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

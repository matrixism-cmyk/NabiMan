import React, { useMemo, useState } from 'react';
import { useApi } from '../../shared/api/useApi';
import { useMecList } from '../../hooks/mec/useMecApi';
import type { MemberUser, ScopeOption } from '../MembershipEditor';
import UserDetail from './UserDetail';
import NewUserForm from './NewUserForm';
import { ago, matches, restrictions, ROLE_LABEL, Role, Status, STATUS_LABEL, STATUS_TONE } from './userModel';
import './users.css';

type StatusFilter = 'all' | Status;
type RoleFilter = 'all' | Role;

const STATUS_CHOICES: StatusFilter[] = ['all', 'active', 'pending', 'suspended', 'rejected'];

/**
 * The super administrator's members: who they are at a glance (level, state,
 * servers, security), found by name, state, level or scope, and everything
 * about one of them in the panel beside the list.
 */
export default function UserAdmin({ me, initialScope = '' }: { me: MemberUser; initialScope?: string }) {
  const { data, loading, error, refetch } = useApi<MemberUser[]>('/api/users');
  const { data: servers } = useApi<{ id: string; name: string }[]>('/api/remote-servers');
  const { data: tenants } = useMecList<{ id: string; display_name: string }>('/api/mec/v1/tenants');
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState<StatusFilter>('all');
  const [role, setRole] = useState<RoleFilter>('all');
  const [scope, setScope] = useState(initialScope);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [message, setMessage] = useState('');

  const options = useMemo<ScopeOption[]>(() => [
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: `서버 · ${s.name}` })),
    ...(tenants || []).map(s => ({ kind: 'tenant' as const, id: s.id, label: `tenant · ${s.display_name}` })),
  ], [servers, tenants]);
  const users = useMemo(() => data || [], [data]);
  const count = (f: (u: MemberUser) => boolean) => users.filter(f).length;
  const list = users.filter(u => matches(u, query)
    && (status === 'all' || u.status === status)
    && (role === 'all' || u.role === role)
    && (!scope || u.memberships.some(m => `${m.kind}:${m.scope_id}` === scope)));
  const selected = users.find(u => u.id === selectedId) || null;
  const label = (m: { kind: string; scope_id: string }) =>
    options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || m.scope_id;

  if (loading && !data) return <div className="panel loading">회원 목록을 불러오는 중…</div>;
  if (error) return <div className="panel error">{error}</div>;

  const stat = (key: string, text: string, n: number, apply: () => void, tone = '') => (
    <button key={key} type="button" className={`users-stat ${tone}`} onClick={apply}>
      <strong>{n}</strong><span>{text}</span>
    </button>
  );

  return (
    <div className="panel users-admin">
      <div className="panel-header">
        <div>
          <h2>최고관리자 · 회원 승인 및 소속 관리</h2>
          <p className="text-secondary users-lead">
            회원을 고르면 오른쪽에서 등급·소속·원격 권한·접속 제한과 로그인 현황을 한 번에 봅니다.
            소속별 정책은 <a href="#administration/scopeSettings">tenant·서버 운영 설정</a>에서 정합니다.
          </p>
        </div>
        <button type="button" className="btn btn-primary" onClick={() => setCreating(true)}>+ 회원 추가</button>
      </div>

      <div className="users-stats">
        {stat('all', '전체 회원', users.length, () => { setStatus('all'); setRole('all'); })}
        {stat('pending', '승인 대기', count(u => u.status === 'pending'), () => setStatus('pending'), count(u => u.status === 'pending') ? 'is-warn' : '')}
        {stat('remote', '원격 전용', count(u => u.role === 'remote'), () => { setRole('remote'); setStatus('all'); })}
        {stat('suspended', '이용 정지', count(u => u.status === 'suspended'), () => setStatus('suspended'))}
        {stat('mfa', '2단계 인증 사용', count(u => Boolean(u.totp_enabled)), () => { setStatus('all'); setRole('all'); })}
      </div>

      {count(u => u.status === 'pending') > 0 && (
        <div className="users-pending" role="note">
          <span>가입 신청 {count(u => u.status === 'pending')}건이 승인을 기다립니다.</span>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => { setStatus('pending'); setScope(''); setRole('all'); }}>
            가입 신청 확인
          </button>
        </div>
      )}
      {message && <div role="status" className="message" onClick={() => setMessage('')}>{message}</div>}

      <div className="users-toolbar">
        <input className="filter-input users-search" type="search" placeholder="아이디 검색" aria-label="아이디 검색"
          value={query} onChange={e => setQuery(e.target.value)} />
        <div className="users-seg" role="radiogroup" aria-label="회원 상태">
          {STATUS_CHOICES.map(s => (
            <button key={s} type="button" role="radio" aria-checked={status === s}
              className={status === s ? 'is-on' : ''} onClick={() => setStatus(s)}>
              {s === 'all' ? '전체' : STATUS_LABEL[s]}
            </button>
          ))}
        </div>
        <select className="select-input" aria-label="회원 등급" value={role} onChange={e => setRole(e.target.value as RoleFilter)}>
          <option value="all">전체 등급</option>
          {(['admin', 'remote', 'viewer', 'operator'] as Role[]).map(r => <option key={r} value={r}>{ROLE_LABEL[r]}</option>)}
        </select>
        <select className="select-input" aria-label="소속 필터" value={scope} onChange={e => setScope(e.target.value)}>
          <option value="">전체 소속</option>
          {options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
        </select>
        <span className="text-secondary users-shown">{list.length}명</span>
      </div>

      <div className={`users-layout ${selected ? 'has-detail' : ''}`}>
        <div className="users-list">
          <table className="data-table users-table">
            <thead>
              <tr><th>아이디</th><th>등급</th><th>상태</th><th>서버·소속</th><th className="users-extra">보안</th><th className="users-extra">최근 로그인</th></tr>
            </thead>
            <tbody>
              {list.map(u => (
                <tr key={u.id} tabIndex={0} aria-selected={u.id === selectedId}
                  className={u.id === selectedId ? 'is-selected' : ''}
                  onClick={() => setSelectedId(u.id)}
                  onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedId(u.id); } }}>
                  <td><strong>{u.username}</strong>{u.id === me.id && <span className="users-me">나</span>}</td>
                  <td><span className={`users-role role-${u.role}`}>{ROLE_LABEL[u.role] || u.role}</span></td>
                  <td><span className={`users-state ${STATUS_TONE[u.status]}`}>{STATUS_LABEL[u.status]}</span></td>
                  <td className="users-scopes">
                    {u.role === 'admin' ? <span className="text-secondary">서비스 전체</span>
                      : u.memberships.length === 0
                        ? <span className="text-secondary">{u.role === 'remote' ? '본인 서버만' : '소속 없음'}</span>
                        : <span title={u.memberships.map(label).join('\n')}>
                          {label(u.memberships[0])}{u.memberships.length > 1 ? ` 외 ${u.memberships.length - 1}` : ''}
                        </span>}
                  </td>
                  <td className="users-extra"><div className="users-security">
                    <span className={u.totp_enabled ? 'users-chip is-ok' : 'users-chip'}>{u.totp_enabled ? '2FA' : '비밀번호'}</span>
                    {restrictions(u).map(r => <span key={r} className="users-chip is-warn">{r}</span>)}
                  </div></td>
                  <td className="text-secondary users-extra">{ago(u.last_login)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {list.length === 0 && <p className="users-empty">조건에 맞는 회원이 없습니다.</p>}
        </div>
        {selected && (
          <UserDetail key={selected.id} user={selected} me={me} options={options}
            onChanged={refetch} onMessage={setMessage}
            onClose={() => setSelectedId(null)} onDeleted={() => { setSelectedId(null); refetch(); }} />
        )}
      </div>

      {creating && (
        <NewUserForm onClose={() => setCreating(false)}
          onCreated={(name) => { setCreating(false); setMessage(`${name} 회원을 만들었습니다. 목록에서 골라 권한을 정하세요.`); setQuery(name); refetch(); }} />
      )}
    </div>
  );
}

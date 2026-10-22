import React, { useMemo, useState } from 'react';
import { apiPost, useApi } from '../../shared/api/useApi';
import { useMecList } from '../../hooks/mec/useMecApi';
import { Membership, MemberUser, ScopeOption, membershipRole } from '../MembershipEditor';
import MemberAccessFields, { accessActive } from '../MemberAccessFields';
import TenantAccessInspection from '../TenantAccessInspection';
import { matches, STATUS_LABEL, STATUS_TONE } from './userModel';
import './users.css';

/**
 * A scope manager's members: the approved accounts in the servers and tenants
 * they manage, with the role, access switch, command permission and expiry
 * of each membership. Approving sign-ups stays with the super administrator.
 */
export default function ScopeMembersPanel({ me, initialScope = '' }: { me: MemberUser; initialScope?: string }) {
  const { data, loading, error, refetch } = useApi<MemberUser[]>('/api/users');
  const { data: servers } = useApi<{ id: string; name: string }[]>('/api/remote-servers');
  const { data: tenants } = useMecList<{ id: string; display_name: string }>('/api/mec/v1/tenants');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [inspection, setInspection] = useState<{ tenantId: string; userId: string } | null>(null);
  const [accessEdit, setAccessEdit] = useState<{ user: MemberUser; membership: Membership } | null>(null);
  const [query, setQuery] = useState('');
  const [scopeFilter, setScopeFilter] = useState(initialScope);
  const [username, setUsername] = useState('');
  const [scope, setScope] = useState(initialScope);
  const [role, setRole] = useState<Membership['role']>('viewer');
  const options = useMemo<ScopeOption[]>(() => [
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: `서버 · ${s.name}` })),
    ...(tenants || []).map(s => ({ kind: 'tenant' as const, id: s.id, label: `tenant · ${s.display_name}` })),
  ], [servers, tenants]);
  const manageable = options.filter(s => me.memberships.some(m => m.kind === s.kind && m.scope_id === s.id && m.role === 'admin' && accessActive(m)));
  const manages = (m: Membership) => manageable.some(s => s.kind === m.kind && s.id === m.scope_id);
  const label = (m: Membership) => options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || m.scope_id;

  const changeMember = async (name: string, membership: Membership, remove = false) => {
    setBusy(true); setMessage('');
    try {
      const res = await apiPost<string>('/api/users/memberships', { username: name, membership, remove });
      setMessage(res.data || res.message);
      if (res.success) { refetch(); setAccessEdit(null); }
    } catch { setMessage('요청을 처리할 수 없습니다. 다시 시도하세요.'); }
    finally { setBusy(false); }
  };

  if (loading && !data) return <div className="panel loading">회원 목록을 불러오는 중…</div>;
  if (error) return <div className="panel error">{error}</div>;
  const list = (data || []).filter(u => matches(u, query) && (!scopeFilter || u.memberships.some(m => `${m.kind}:${m.scope_id}` === scopeFilter)));

  return (
    <div className="panel users-admin">
      <div className="panel-header">
        <div>
          <h2>소속 회원 관리</h2>
          <p className="text-secondary users-lead">내가 관리하는 소속의 승인된 회원에게 운영·조회 권한을 줍니다. 가입 승인은 최고관리자에게 요청하세요.
            소속별 정책은 <a href="#administration/scopeSettings">tenant·서버 운영 설정</a>에서 봅니다.</p>
        </div>
      </div>
      {message && <div role="status" className="message" onClick={() => setMessage('')}>{message}</div>}

      {manageable.length > 0 && (
        <form className="users-card users-add" onSubmit={e => {
          e.preventDefault();
          const s = manageable.find(o => `${o.kind}:${o.id}` === scope);
          if (s) changeMember(username, { kind: s.kind, scope_id: s.id, role });
        }}>
          <h4>승인된 회원을 소속에 추가</h4>
          <div className="users-row">
            <input aria-label="추가할 회원 아이디" className="filter-input" placeholder="회원 아이디" required value={username} onChange={e => setUsername(e.target.value)} />
            <select aria-label="관리 소속" className="select-input" required value={scope} onChange={e => setScope(e.target.value)}>
              <option value="">소속 선택</option>{manageable.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
            </select>
            <select aria-label="추가할 회원 권한" className="select-input" value={role} onChange={e => setRole(e.target.value as Membership['role'])}>
              {Object.entries(membershipRole).filter(([r]) => r !== 'admin').map(([r, text]) => <option key={r} value={r}>{text}</option>)}
            </select>
            <button className="btn btn-primary" disabled={busy}>소속 추가</button>
          </div>
        </form>
      )}

      {accessEdit && (
        <div className="users-card">
          <h4>{accessEdit.user.username} · 소속 접근 설정</h4>
          <MemberAccessFields value={accessEdit.membership} onChange={membership => setAccessEdit({ ...accessEdit, membership })} />
          <div className="btn-group">
            <button className="btn btn-primary" disabled={busy} onClick={() => changeMember(accessEdit.user.username, accessEdit.membership)}>접근 설정 저장</button>
            <button className="btn btn-secondary" onClick={() => setAccessEdit(null)}>취소</button>
          </div>
        </div>
      )}
      {inspection && <TenantAccessInspection key={`${inspection.tenantId}:${inspection.userId}`} {...inspection}
        onClose={() => setInspection(null)} onInspected={refetch} />}

      <div className="users-toolbar">
        <input className="filter-input users-search" type="search" placeholder="아이디 검색" aria-label="아이디 검색" value={query} onChange={e => setQuery(e.target.value)} />
        <select className="select-input" aria-label="소속 필터" value={scopeFilter} onChange={e => setScopeFilter(e.target.value)}>
          <option value="">전체 소속</option>{options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
        </select>
        <span className="text-secondary users-shown">{list.length}명</span>
      </div>

      <table className="data-table users-table">
        <thead><tr><th>아이디</th><th>상태</th><th>소속 및 권한</th></tr></thead>
        <tbody>{list.map(u => (
          <tr key={u.id}>
            <td><strong>{u.username}</strong>{u.id === me.id && <span className="users-me">나</span>}</td>
            <td><span className={`users-state ${STATUS_TONE[u.status]}`}>{STATUS_LABEL[u.status]}</span></td>
            <td>
              {u.memberships.length === 0 && <span className="text-secondary">소속 없음</span>}
              {u.memberships.map(m => (
                <div key={`${m.kind}:${m.scope_id}`} className="users-membership">
                  <span>{label(m)} · {membershipRole[m.role]}</span>
                  <span className="text-secondary">{accessActive(m) ? '접근 허용' : '접근 차단'}
                    {m.kind === 'server' && m.role !== 'viewer' && m.allow_exec === false ? ' · 명령 실행 차단' : ''}
                    {m.expires_at ? ` · 만료 ${new Date(m.expires_at).toLocaleString()}` : ''}</span>
                  {m.kind === 'tenant' && u.id !== me.id && m.role !== 'admin' && manages(m) && (
                    <button className="btn btn-sm btn-secondary" onClick={() => setInspection({ tenantId: m.scope_id, userId: u.id })}>실제 접근 확인</button>
                  )}
                  {u.id !== me.id && m.role !== 'admin' && manages(m) && <>
                    <select aria-label={`${u.username} 소속 권한`} className="select-input" value={m.role} disabled={busy}
                      onChange={e => changeMember(u.username, { ...m, role: e.target.value as Membership['role'] })}>
                      <option value="viewer">조회자</option><option value="operator">운영자</option>
                    </select>
                    <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => setAccessEdit({ user: u, membership: { ...m } })}>접근 설정</button>
                    <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => changeMember(u.username, m, true)}>소속 해제</button>
                  </>}
                </div>
              ))}
            </td>
          </tr>
        ))}</tbody>
      </table>
      {list.length === 0 && <p className="users-empty">표시할 회원이 없습니다.</p>}
    </div>
  );
}

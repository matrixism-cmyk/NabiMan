import React, { useMemo, useState } from 'react';
import { useApi, apiPost, apiRequest } from '../shared/api/useApi';
import { useMecList } from '../hooks/mec/useMecApi';
import MembershipEditor, { Membership, MemberUser, ScopeOption, membershipRole } from './MembershipEditor';

import MemberAccessFields, { accessActive } from './MemberAccessFields';
import ExternalIdentityEditor from './ExternalIdentityEditor';
import TenantAccessInspection from './TenantAccessInspection';
import { confirmDialog } from '../shared/ui/ConfirmDialog';

const statusLabel = { active: '승인', pending: '승인 대기', rejected: '반려', suspended: '이용 정지' };
export default function UserManagementPanel({ initialScope = '' }: { initialScope?: string }) {
  const { data: me } = useApi<MemberUser>('/api/users/me');
  const { data, loading, error, refetch } = useApi<MemberUser[]>('/api/users');
  const { data: servers } = useApi<{ id: string; name: string }[]>('/api/remote-servers');
  const { data: tenants } = useMecList<{ id: string; display_name: string }>('/api/mec/v1/tenants');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState<MemberUser | null>(null);
  const [identityUser, setIdentityUser] = useState<MemberUser | null>(null);
  const [inspection, setInspection] = useState<{ tenantId: string; userId: string } | null>(null);
  const [draft, setDraft] = useState<Membership[]>([]);
  const [scopeFilter, setScopeFilter] = useState(initialScope);
  const [accessEdit, setAccessEdit] = useState<{ user: MemberUser; membership: Membership } | null>(null);
  const [filter, setFilter] = useState('all');
  const [username, setUsername] = useState('');
  const [scope, setScope] = useState(initialScope);
  const [role, setRole] = useState<Membership['role']>('viewer');
  const superAdmin = me?.role === 'admin';
  const options = useMemo<ScopeOption[]>(() => [
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: `서버 · ${s.name}` })),
    ...(tenants || []).map(s => ({ kind: 'tenant' as const, id: s.id, label: `tenant · ${s.display_name}` })),
  ], [servers, tenants]);
  const manageable = options.filter(s => superAdmin || me?.memberships.some(m => m.kind === s.kind && m.scope_id === s.id && m.role === 'admin' && accessActive(m)));
  const act = async (operation: () => ReturnType<typeof apiPost<string>>) => {
    setBusy(true); setMessage('');
    try { const res = await operation(); setMessage(res.data || res.message); if (res.success) { refetch(); setEditing(null); setAccessEdit(null); } }
    catch { setMessage('요청을 처리할 수 없습니다. 다시 시도하세요.'); }
    finally { setBusy(false); }
  };
  const review = (u: MemberUser, status: MemberUser['status'], memberships = u.memberships) =>
    act(() => apiPost(`/api/users/${u.id}/review`, { status, memberships }));
  const changeMember = (name: string, membership: Membership, remove = false) =>
    act(() => apiPost('/api/users/memberships', { username: name, membership, remove }));
  if (loading) return <div className="panel loading">회원 목록을 불러오는 중…</div>;
  if (error) return <div className="panel error">{error}</div>;
  return <div className="panel">
    <div className="panel-header"><h2>{superAdmin ? '최고관리자 · 회원 승인 및 소속 관리' : '소속 회원 관리'}</h2></div>
    <p>{superAdmin ? '가입 신청을 승인하고 서버·tenant별 관리자를 지정합니다.' : '내가 관리하는 소속의 승인된 회원에게 운영·조회 권한을 부여합니다.'}</p>
    <p><a href="#administration/scopeSettings">tenant·서버 운영 설정</a>에서 소속별 정책과 자원 할당량을 확인할 수 있습니다. 가입 승인은 최고관리자에게 요청하세요.</p>
    {superAdmin && <div className="form-row"><strong>승인 대기 {data?.filter(u => u.status === 'pending').length || 0}명</strong>
      <button className="btn btn-secondary" onClick={() => { setFilter('pending'); setScopeFilter(''); }}>가입 신청 확인</button></div>}
    {message && <div role="status" className="message">{message}</div>}
    {manageable.length > 0 && <form className="inline-form" onSubmit={e => {
      e.preventDefault(); const s = manageable.find(o => `${o.kind}:${o.id}` === scope);
      if (s) changeMember(username, { kind: s.kind, scope_id: s.id, role });
    }}>
      <h3>승인된 회원을 소속에 추가</h3>
      <div className="form-row">
        <input aria-label="추가할 회원 아이디" placeholder="회원 아이디" required value={username} onChange={e => setUsername(e.target.value)} />
        <select aria-label="관리 소속" className="select-input" required value={scope} onChange={e => setScope(e.target.value)}>
          <option value="">소속 선택</option>{manageable.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
        </select>
        <select aria-label="추가할 회원 권한" className="select-input" value={role} onChange={e => setRole(e.target.value as Membership['role'])}>
          {Object.entries(membershipRole).filter(([r]) => superAdmin || r !== 'admin').map(([r, label]) => <option key={r} value={r}>{label}</option>)}
        </select>
        <button className="btn btn-primary" disabled={busy}>소속 추가</button>
      </div>
    </form>}
    {superAdmin && identityUser && <ExternalIdentityEditor key={identityUser.id} user={identityUser} onClose={() => setIdentityUser(null)} />}
    {inspection && <TenantAccessInspection key={`${inspection.tenantId}:${inspection.userId}`} {...inspection} onClose={() => setInspection(null)} onInspected={refetch} />}
    {editing && <div className="inline-form">
      <h3>{editing.username} · 승인 및 소속 설정</h3>
      <MembershipEditor options={options} value={draft} onChange={setDraft} />
      <div className="btn-group">
        <button className="btn btn-primary" disabled={busy} onClick={() => review(editing, 'active', draft)}>승인하고 저장</button>
        <button className="btn btn-secondary" disabled={busy} onClick={() => setEditing(null)}>취소</button>
      </div>
    </div>}
    {accessEdit && <div className="inline-form">
      <h3>{accessEdit.user.username} · 소속 접근 설정</h3>
      <MemberAccessFields value={accessEdit.membership} onChange={membership => setAccessEdit({ ...accessEdit, membership })} />
      <button className="btn btn-primary" disabled={busy} onClick={() => changeMember(accessEdit.user.username, accessEdit.membership)}>접근 설정 저장</button>
      <button className="btn btn-secondary" onClick={() => setAccessEdit(null)}>취소</button>
    </div>}
    <div className="form-row"><label>소속 필터 <select className="select-input" value={scopeFilter} onChange={e => setScopeFilter(e.target.value)}>
      <option value="">전체 소속</option>{options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
    </select></label><label>회원 상태 <select className="select-input" value={filter} onChange={e => setFilter(e.target.value)}>
      <option value="all">전체</option>{Object.entries(statusLabel).map(([s, label]) => <option key={s} value={s}>{label}</option>)}
    </select></label></div>
    <table className="data-table"><thead><tr><th>아이디</th><th>상태</th><th>소속 및 권한</th><th>관리</th></tr></thead>
      <tbody>{data?.filter(u => (filter === 'all' || u.status === filter) && (!scopeFilter || u.memberships.some(m => `${m.kind}:${m.scope_id}` === scopeFilter))).map(u => <tr key={u.id}>
        <td><strong>{u.username}</strong>{u.role === 'admin' && <span> · 최고관리자</span>}
          {superAdmin && u.status === 'active' && <select className="select-input" aria-label={`${u.username} 서비스 권한`}
            value={u.role} disabled={busy} onChange={async e => {
              const nextRole = e.target.value;
              if (nextRole === 'admin' && !(await confirmDialog({ message: `${u.username}에게 서비스 전체 최고관리자 권한을 부여할까요?`, danger: true }))) return;
              act(() => apiRequest(`/api/users/${u.id}`, { method: 'PUT', body: JSON.stringify({ role: nextRole }) }));
            }}>
            <option value="viewer">일반 회원 (소속별 권한 적용)</option>
            {u.role === 'operator' && <option value="operator">기존 운영자</option>}
            <option value="admin">최고관리자</option>
          </select>}
        </td>
        <td>{statusLabel[u.status]}</td>
        <td>{u.memberships.map(m => <div key={`${m.kind}:${m.scope_id}`}>
          {options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || m.scope_id} · {membershipRole[m.role]}
          <span> · {accessActive(m) ? '접근 허용' : '접근 차단'}{m.kind === 'server' && m.role !== 'viewer' && m.allow_exec === false ? ' · 명령 실행 차단' : ''}{m.expires_at ? ` · 만료 ${new Date(m.expires_at).toLocaleString()}` : ''}</span>
          {m.kind === 'tenant' && (superAdmin || u.id !== me?.id && m.role !== 'admin' && manageable.some(s => s.kind === 'tenant' && s.id === m.scope_id)) && <button className="btn btn-sm btn-secondary" onClick={() => setInspection({ tenantId: m.scope_id, userId: u.id })}>실제 접근 확인</button>}
          {!superAdmin && u.id !== me?.id && m.role !== 'admin' && manageable.some(s => s.kind === m.kind && s.id === m.scope_id) && <>
            <select aria-label={`${u.username} 소속 권한`} className="select-input" value={m.role} disabled={busy}
              onChange={e => changeMember(u.username, { ...m, role: e.target.value as Membership['role'] })}>
              <option value="viewer">조회자</option><option value="operator">운영자</option>
            </select>
            <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => setAccessEdit({ user: u, membership: { ...m } })}>접근 설정</button>
            <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => changeMember(u.username, m, true)}>소속 해제</button>
          </>}
        </div>)}{u.memberships.length === 0 && (u.role === 'admin' ? '서비스 전체' : u.scope_restricted ? '소속 미지정' : '기존 서비스 권한')}</td>
        <td>{superAdmin && <div className="btn-group">
          {u.status === 'active' && <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => setIdentityUser(u)}>Rancher 계정 연결</button>}
          <button className="btn btn-sm btn-primary" disabled={busy} onClick={() => { setEditing(u); setDraft(u.status === 'pending' ? u.requested_memberships : u.memberships); }}>소속 설정{u.status === 'pending' ? ' / 승인' : ''}</button>
          {u.status === 'pending' && <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => review(u, 'rejected', [])}>반려</button>}
          {u.status === 'active' && u.id !== me?.id && <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => review(u, 'suspended')}>이용 정지</button>}
          {u.id !== me?.id && <button className="btn btn-sm btn-danger" disabled={busy} onClick={async () => {
            if (await confirmDialog({ message: `${u.username} 회원을 삭제할까요?`, danger: true, confirmLabel: '삭제' })) act(() => apiRequest(`/api/users/${u.id}`, { method: 'DELETE' }));
          }}>삭제</button>}
        </div>}</td>
      </tr>)}</tbody>
    </table>
    {data?.length === 0 && <p>표시할 회원이 없습니다.</p>}
  </div>;
}

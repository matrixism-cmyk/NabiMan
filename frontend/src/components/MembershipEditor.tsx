import React from 'react';
import MemberAccessFields from './MemberAccessFields';
export interface Membership { kind: 'server' | 'tenant'; scope_id: string; role: 'admin' | 'operator' | 'viewer'; enabled?: boolean; allow_exec?: boolean | null; expires_at?: string | null }
export interface MemberUser {
  mfa_blocked?: boolean;
  id: string; username: string; role: string; status: 'active' | 'pending' | 'rejected' | 'suspended';
  memberships: Membership[]; requested_memberships: Membership[]; scope_restricted: boolean;
  created_at: string; last_login: string | null;
}
export interface ScopeOption { kind: Membership['kind']; id: string; label: string }
export const membershipRole = { admin: '소속 관리자', operator: '운영자', viewer: '조회자' };
/** `remote`: a remote-only member's assigned servers — servers only, no role to pick. */
export default function MembershipEditor({ value, onChange, options, remote = false }: {
  value: Membership[]; onChange: (m: Membership[]) => void; options: ScopeOption[]; remote?: boolean;
}) {
  const available = options.filter(s => (!remote || s.kind === 'server') && !value.some(m => m.kind === s.kind && m.scope_id === s.id));
  return <div>
    {value.map((m, i) => <div className="inline-form" key={`${m.kind}:${m.scope_id}`}>
      <span>{options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || `${m.kind}: ${m.scope_id}`}</span>
      {!remote && <select aria-label="소속 권한" className="select-input" value={m.role}
        onChange={e => onChange(value.map((a, j) => j === i ? { ...a, role: e.target.value as Membership['role'] } : a))}>
        {Object.entries(membershipRole).map(([r, label]) => <option key={r} value={r}>{label}</option>)}
      </select>}
      <button type="button" className="btn btn-sm btn-secondary" onClick={() => onChange(value.filter((_, j) => j !== i))}>제거</button>
      <MemberAccessFields value={m} remote={remote} onChange={next => onChange(value.map((a, j) => j === i ? next : a))} />
    </div>)}
    <select aria-label="소속 추가" className="select-input" value="" onChange={e => {
      const s = available.find(a => `${a.kind}:${a.id}` === e.target.value);
      if (s) onChange([...value, { kind: s.kind, scope_id: s.id, role: 'viewer' }]);
    }}>
      <option value="">{remote ? '쓸 수 있게 할 서버 추가' : '서버 또는 tenant 소속 추가'}</option>
      {available.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
    </select>
  </div>;
}

import React from 'react';
import MemberAccessFields from './MemberAccessFields';
import type { MemberGrants } from './remote/member';
import { useT } from '../i18n';
export interface Membership { kind: 'server' | 'tenant'; scope_id: string; role: 'admin' | 'operator' | 'viewer'; enabled?: boolean; allow_exec?: boolean | null; expires_at?: string | null }
export interface MemberUser {
  mfa_blocked?: boolean;
  id: string; username: string; role: string; status: 'active' | 'pending' | 'rejected' | 'suspended';
  memberships: Membership[]; requested_memberships: Membership[]; scope_restricted: boolean;
  /** What a remote-only member may do beyond working on their servers. */
  remote_permissions?: MemberGrants;
  access?: AccessRules;
  totp_enabled?: boolean;
  created_at: string; last_login: string | null;
}
export type { MemberGrants as RemotePermissions } from './remote/member';
/** When and from where an account may be used (the account's `access`). */
export interface AccessRules {
  expires_at?: string | null;
  require_mfa: boolean;
  allowed_networks: string[];
  allowed_hours?: { start: number; end: number; days: number[] } | null;
}
export interface ScopeOption { kind: Membership['kind']; id: string; label: string }
/** Translation keys of each membership role's name. */
export const membershipRole = { admin: 'membership.role.admin', operator: 'membership.role.operator', viewer: 'membership.role.viewer' };
/** `remote`: a remote-only member's assigned servers — servers only, no role to pick. */
export default function MembershipEditor({ value, onChange, options, remote = false }: {
  value: Membership[]; onChange: (m: Membership[]) => void; options: ScopeOption[]; remote?: boolean;
}) {
  const { t } = useT();
  const available = options.filter(s => (!remote || s.kind === 'server') && !value.some(m => m.kind === s.kind && m.scope_id === s.id));
  return <div>
    {value.map((m, i) => <div className="inline-form" key={`${m.kind}:${m.scope_id}`}>
      <span>{options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || `${m.kind}: ${m.scope_id}`}</span>
      {!remote && <select aria-label={t('membership.roleLabel')} className="select-input" value={m.role}
        onChange={e => onChange(value.map((a, j) => j === i ? { ...a, role: e.target.value as Membership['role'] } : a))}>
        {Object.entries(membershipRole).map(([r, label]) => <option key={r} value={r}>{t(label)}</option>)}
      </select>}
      <button type="button" className="btn btn-sm btn-secondary" onClick={() => onChange(value.filter((_, j) => j !== i))}>{t('membership.remove')}</button>
      <MemberAccessFields value={m} remote={remote} onChange={next => onChange(value.map((a, j) => j === i ? next : a))} />
    </div>)}
    <select aria-label={t('membership.add')} className="select-input" value="" onChange={e => {
      const s = available.find(a => `${a.kind}:${a.id}` === e.target.value);
      if (s) onChange([...value, { kind: s.kind, scope_id: s.id, role: 'viewer' }]);
    }}>
      <option value="">{remote ? t('membership.addServer') : t('membership.addScope')}</option>
      {available.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
    </select>
  </div>;
}

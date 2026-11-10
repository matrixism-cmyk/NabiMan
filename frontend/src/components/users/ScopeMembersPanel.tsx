import React, { useMemo, useState } from 'react';
import { apiPost, useApi } from '../../shared/api/useApi';
import { useMecList } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
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
  const { t } = useT();
  const [busy, setBusy] = useState(false);
  const [inspection, setInspection] = useState<{ tenantId: string; userId: string } | null>(null);
  const [accessEdit, setAccessEdit] = useState<{ user: MemberUser; membership: Membership } | null>(null);
  const [query, setQuery] = useState('');
  const [scopeFilter, setScopeFilter] = useState(initialScope);
  const [username, setUsername] = useState('');
  const [scope, setScope] = useState(initialScope);
  const [role, setRole] = useState<Membership['role']>('viewer');
  const options = useMemo<ScopeOption[]>(() => [
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: t('users.scope.server', { name: s.name }) })),
    ...(tenants || []).map(s => ({ kind: 'tenant' as const, id: s.id, label: t('users.scope.tenant', { name: s.display_name }) })),
  ], [servers, tenants, t]);
  const manageable = options.filter(s => me.memberships.some(m => m.kind === s.kind && m.scope_id === s.id && m.role === 'admin' && accessActive(m)));
  const manages = (m: Membership) => manageable.some(s => s.kind === m.kind && s.id === m.scope_id);
  const label = (m: Membership) => options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || m.scope_id;

  const changeMember = async (name: string, membership: Membership, remove = false) => {
    setBusy(true); setMessage('');
    try {
      const res = await apiPost<string>('/api/users/memberships', { username: name, membership, remove });
      setMessage(res.data || res.message);
      if (res.success) { refetch(); setAccessEdit(null); }
    } catch { setMessage(t('users.requestFailed')); }
    finally { setBusy(false); }
  };

  if (loading && !data) return <div className="panel loading">{t('users.loading')}</div>;
  if (error) return <div className="panel error">{error}</div>;
  const list = (data || []).filter(u => matches(u, query) && (!scopeFilter || u.memberships.some(m => `${m.kind}:${m.scope_id}` === scopeFilter)));

  return (
    <div className="panel users-admin">
      <div className="panel-header">
        <div>
          <h2>{t('scopeMembers.title')}</h2>
          <p className="text-secondary users-lead">{t('scopeMembers.lead')}{' '}
            {t('scopeMembers.policyBefore')}<a href="#administration/scopeSettings">{t('users.scopeSettingsLink')}</a>{t('scopeMembers.policyAfter')}</p>
        </div>
      </div>
      {message && <div role="status" className="message" onClick={() => setMessage('')}>{message}</div>}

      {manageable.length > 0 && (
        <form className="users-card users-add" onSubmit={e => {
          e.preventDefault();
          const s = manageable.find(o => `${o.kind}:${o.id}` === scope);
          if (s) changeMember(username, { kind: s.kind, scope_id: s.id, role });
        }}>
          <h4>{t('scopeMembers.addTitle')}</h4>
          <div className="users-row">
            <input aria-label={t('scopeMembers.addId')} className="filter-input" placeholder={t('scopeMembers.idPlaceholder')} required value={username} onChange={e => setUsername(e.target.value)} />
            <select aria-label={t('scopeMembers.managedScope')} className="select-input" required value={scope} onChange={e => setScope(e.target.value)}>
              <option value="">{t('scopeMembers.pickScope')}</option>{manageable.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
            </select>
            <select aria-label={t('scopeMembers.addRole')} className="select-input" value={role} onChange={e => setRole(e.target.value as Membership['role'])}>
              {Object.entries(membershipRole).filter(([r]) => r !== 'admin').map(([r, text]) => <option key={r} value={r}>{t(text)}</option>)}
            </select>
            <button className="btn btn-primary" disabled={busy}>{t('membership.add')}</button>
          </div>
        </form>
      )}

      {accessEdit && (
        <div className="users-card">
          <h4>{t('scopeMembers.accessTitle', { name: accessEdit.user.username })}</h4>
          <MemberAccessFields value={accessEdit.membership} onChange={membership => setAccessEdit({ ...accessEdit, membership })} />
          <div className="btn-group">
            <button className="btn btn-primary" disabled={busy} onClick={() => changeMember(accessEdit.user.username, accessEdit.membership)}>{t('scopeMembers.saveAccess')}</button>
            <button className="btn btn-secondary" onClick={() => setAccessEdit(null)}>{t('common.cancel')}</button>
          </div>
        </div>
      )}
      {inspection && <TenantAccessInspection key={`${inspection.tenantId}:${inspection.userId}`} {...inspection}
        onClose={() => setInspection(null)} onInspected={refetch} />}

      <div className="users-toolbar">
        <input className="filter-input users-search" type="search" placeholder={t('users.searchId')} aria-label={t('users.searchId')} value={query} onChange={e => setQuery(e.target.value)} />
        <select className="select-input" aria-label={t('users.scopeFilter')} value={scopeFilter} onChange={e => setScopeFilter(e.target.value)}>
          <option value="">{t('users.allScopes')}</option>{options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
        </select>
        <span className="text-secondary users-shown">{t('users.count', { n: list.length })}</span>
      </div>

      <table className="data-table users-table">
        <thead><tr><th>{t('users.col.id')}</th><th>{t('users.col.status')}</th><th>{t('scopeMembers.colScopes')}</th></tr></thead>
        <tbody>{list.map(u => (
          <tr key={u.id}>
            <td><strong>{u.username}</strong>{u.id === me.id && <span className="users-me">{t('users.me')}</span>}</td>
            <td><span className={`users-state ${STATUS_TONE[u.status]}`}>{t(STATUS_LABEL[u.status])}</span></td>
            <td>
              {u.memberships.length === 0 && <span className="text-secondary">{t('users.noScope')}</span>}
              {u.memberships.map(m => (
                <div key={`${m.kind}:${m.scope_id}`} className="users-membership">
                  <span>{label(m)} · {t(membershipRole[m.role])}</span>
                  <span className="text-secondary">{accessActive(m) ? t('scopeMembers.allowed') : t('scopeMembers.blocked')}
                    {m.kind === 'server' && m.role !== 'viewer' && m.allow_exec === false ? t('scopeMembers.execBlocked') : ''}
                    {m.expires_at ? t('scopeMembers.expires', { when: new Date(m.expires_at).toLocaleString() }) : ''}</span>
                  {m.kind === 'tenant' && u.id !== me.id && m.role !== 'admin' && manages(m) && (
                    <button className="btn btn-sm btn-secondary" onClick={() => setInspection({ tenantId: m.scope_id, userId: u.id })}>{t('users.inspectAccess')}</button>
                  )}
                  {u.id !== me.id && m.role !== 'admin' && manages(m) && <>
                    <select aria-label={t('scopeMembers.roleOf', { name: u.username })} className="select-input" value={m.role} disabled={busy}
                      onChange={e => changeMember(u.username, { ...m, role: e.target.value as Membership['role'] })}>
                      <option value="viewer">{t('membership.role.viewer')}</option><option value="operator">{t('membership.role.operator')}</option>
                    </select>
                    <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => setAccessEdit({ user: u, membership: { ...m } })}>{t('scopeMembers.access')}</button>
                    <button className="btn btn-sm btn-secondary" disabled={busy} onClick={() => changeMember(u.username, m, true)}>{t('scopeMembers.leave')}</button>
                  </>}
                </div>
              ))}
            </td>
          </tr>
        ))}</tbody>
      </table>
      {list.length === 0 && <p className="users-empty">{t('scopeMembers.empty')}</p>}
    </div>
  );
}

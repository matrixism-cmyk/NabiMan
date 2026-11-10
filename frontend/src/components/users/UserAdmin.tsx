import React, { useMemo, useState } from 'react';
import { useApi } from '../../shared/api/useApi';
import { useMecList } from '../../hooks/mec/useMecApi';
import { useT } from '../../i18n';
import type { MemberUser, ScopeOption } from '../MembershipEditor';
import UserDetail from './UserDetail';
import NewUserForm from './NewUserForm';
import { ago, matches, restrictions, roleName, Role, Status, STATUS_LABEL, STATUS_TONE } from './userModel';
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
  const { t } = useT();

  const options = useMemo<ScopeOption[]>(() => [
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: t('users.scope.server', { name: s.name }) })),
    ...(tenants || []).map(s => ({ kind: 'tenant' as const, id: s.id, label: t('users.scope.tenant', { name: s.display_name }) })),
  ], [servers, tenants, t]);
  const users = useMemo(() => data || [], [data]);
  const count = (f: (u: MemberUser) => boolean) => users.filter(f).length;
  const list = users.filter(u => matches(u, query)
    && (status === 'all' || u.status === status)
    && (role === 'all' || u.role === role)
    && (!scope || u.memberships.some(m => `${m.kind}:${m.scope_id}` === scope)));
  const selected = users.find(u => u.id === selectedId) || null;
  const label = (m: { kind: string; scope_id: string }) =>
    options.find(s => s.kind === m.kind && s.id === m.scope_id)?.label || m.scope_id;

  if (loading && !data) return <div className="panel loading">{t('users.loading')}</div>;
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
          <h2>{t('users.admin.title')}</h2>
          <p className="text-secondary users-lead">
            {t('users.admin.lead')}{' '}
            {t('users.admin.policyBefore')}<a href="#administration/scopeSettings">{t('users.scopeSettingsLink')}</a>{t('users.admin.policyAfter')}
          </p>
        </div>
        <button type="button" className="btn btn-primary" onClick={() => setCreating(true)}>{t('users.admin.add')}</button>
      </div>

      <div className="users-stats">
        {stat('all', t('users.admin.statAll'), users.length, () => { setStatus('all'); setRole('all'); })}
        {stat('pending', t('users.status.pending'), count(u => u.status === 'pending'), () => setStatus('pending'), count(u => u.status === 'pending') ? 'is-warn' : '')}
        {stat('remote', t('users.admin.statRemote'), count(u => u.role === 'remote'), () => { setRole('remote'); setStatus('all'); })}
        {stat('suspended', t('users.status.suspended'), count(u => u.status === 'suspended'), () => setStatus('suspended'))}
        {stat('mfa', t('users.admin.statMfa'), count(u => Boolean(u.totp_enabled)), () => { setStatus('all'); setRole('all'); })}
      </div>

      {count(u => u.status === 'pending') > 0 && (
        <div className="users-pending" role="note">
          <span>{t('users.admin.pendingNote', { n: count(u => u.status === 'pending') })}</span>
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => { setStatus('pending'); setScope(''); setRole('all'); }}>
            {t('users.admin.pendingReview')}
          </button>
        </div>
      )}
      {message && <div role="status" className="message" onClick={() => setMessage('')}>{message}</div>}

      <div className="users-toolbar">
        <input className="filter-input users-search" type="search" placeholder={t('users.searchId')} aria-label={t('users.searchId')}
          value={query} onChange={e => setQuery(e.target.value)} />
        <div className="users-seg" role="radiogroup" aria-label={t('users.admin.statusGroup')}>
          {STATUS_CHOICES.map(s => (
            <button key={s} type="button" role="radio" aria-checked={status === s}
              className={status === s ? 'is-on' : ''} onClick={() => setStatus(s)}>
              {s === 'all' ? t('users.admin.all') : t(STATUS_LABEL[s])}
            </button>
          ))}
        </div>
        <select className="select-input" aria-label={t('users.admin.roleFilter')} value={role} onChange={e => setRole(e.target.value as RoleFilter)}>
          <option value="all">{t('users.admin.allRoles')}</option>
          {(['admin', 'remote', 'viewer', 'operator'] as Role[]).map(r => <option key={r} value={r}>{roleName(t, r)}</option>)}
        </select>
        <select className="select-input" aria-label={t('users.scopeFilter')} value={scope} onChange={e => setScope(e.target.value)}>
          <option value="">{t('users.allScopes')}</option>
          {options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
        </select>
        <span className="text-secondary users-shown">{t('users.count', { n: list.length })}</span>
      </div>

      <div className={`users-layout ${selected ? 'has-detail' : ''}`}>
        <div className="users-list">
          <table className="data-table users-table">
            <thead>
              <tr><th>{t('users.col.id')}</th><th>{t('users.admin.colRole')}</th><th>{t('users.col.status')}</th><th>{t('users.admin.colScopes')}</th><th className="users-extra">{t('users.admin.colSecurity')}</th><th className="users-extra">{t('users.admin.colLastLogin')}</th></tr>
            </thead>
            <tbody>
              {list.map(u => (
                <tr key={u.id} tabIndex={0} aria-selected={u.id === selectedId}
                  className={u.id === selectedId ? 'is-selected' : ''}
                  onClick={() => setSelectedId(u.id)}
                  onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setSelectedId(u.id); } }}>
                  <td><strong>{u.username}</strong>{u.id === me.id && <span className="users-me">{t('users.me')}</span>}</td>
                  <td><span className={`users-role role-${u.role}`}>{roleName(t, u.role)}</span></td>
                  <td><span className={`users-state ${STATUS_TONE[u.status]}`}>{t(STATUS_LABEL[u.status])}</span></td>
                  <td className="users-scopes">
                    {u.role === 'admin' ? <span className="text-secondary">{t('users.admin.wholeService')}</span>
                      : u.memberships.length === 0
                        ? <span className="text-secondary">{u.role === 'remote' ? t('users.admin.ownServersOnly') : t('users.noScope')}</span>
                        : <span title={u.memberships.map(label).join('\n')}>
                          {label(u.memberships[0])}{u.memberships.length > 1 ? t('users.admin.andMore', { n: u.memberships.length - 1 }) : ''}
                        </span>}
                  </td>
                  <td className="users-extra"><div className="users-security">
                    <span className={u.totp_enabled ? 'users-chip is-ok' : 'users-chip'}>{u.totp_enabled ? '2FA' : t('users.admin.password')}</span>
                    {restrictions(t, u).map(r => <span key={r} className="users-chip is-warn">{r}</span>)}
                  </div></td>
                  <td className="text-secondary users-extra">{ago(t, u.last_login)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {list.length === 0 && <p className="users-empty">{t('users.admin.empty')}</p>}
        </div>
        {selected && (
          <UserDetail key={selected.id} user={selected} me={me} options={options}
            onChanged={refetch} onMessage={setMessage}
            onClose={() => setSelectedId(null)} onDeleted={() => { setSelectedId(null); refetch(); }} />
        )}
      </div>

      {creating && (
        <NewUserForm onClose={() => setCreating(false)}
          onCreated={(name) => { setCreating(false); setMessage(t('users.admin.created', { name })); setQuery(name); refetch(); }} />
      )}
    </div>
  );
}

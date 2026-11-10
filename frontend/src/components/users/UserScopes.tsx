import React, { useState } from 'react';
import MembershipEditor, { Membership, MemberUser, ScopeOption } from '../MembershipEditor';
import TenantAccessInspection from '../TenantAccessInspection';
import { useT } from '../../i18n';

interface Props {
  user: MemberUser;
  options: ScopeOption[];
  busy: boolean;
  review: (status: MemberUser['status'], memberships: Membership[]) => Promise<boolean>;
}

/**
 * Where the account belongs: servers and tenants with a role in each, or for
 * a remote-only member, the administrators' servers they may use.
 */
export default function UserScopes({ user, options, busy, review }: Props) {
  const pending = user.status === 'pending';
  const remote = user.role === 'remote';
  const [draft, setDraft] = useState<Membership[]>(pending ? user.requested_memberships : user.memberships);
  const [inspecting, setInspecting] = useState<string | null>(null);
  const { t } = useT();
  const changed = JSON.stringify(draft) !== JSON.stringify(pending ? null : user.memberships);

  return (
    <div className="users-sections">
      <section className="users-card">
        <h4>{remote ? t('users.scopes.assignedTitle') : t('users.scopes.title')}</h4>
        <p className="text-secondary">
          {remote
            ? t('users.scopes.assignedHint')
            : t('users.scopes.hint')}
        </p>
        {pending && user.requested_memberships.length > 0 && (
          <p className="text-secondary">{t('users.scopes.prefilled')}</p>
        )}
        <MembershipEditor options={options} value={draft} onChange={setDraft} remote={remote} />
        <div className="btn-group">
          {pending
            ? <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => review('active', draft)}>{t('users.scopes.approveSave')}</button>
            : <button type="button" className="btn btn-primary btn-sm" disabled={busy || !changed} onClick={() => review(user.status, draft)}>
              {remote ? t('users.scopes.saveAssigned') : t('users.scopes.save')}
            </button>}
          {!pending && changed && (
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDraft(user.memberships)}>{t('users.revert')}</button>
          )}
        </div>
        <p className="text-secondary">{t('users.scopes.saveNote')}</p>
      </section>

      {user.memberships.some(m => m.kind === 'tenant') && (
        <section className="users-card">
          <h4>{t('users.scopes.tenantAccess')}</h4>
          <ul className="users-lines">
            {user.memberships.filter(m => m.kind === 'tenant').map(m => (
              <li key={m.scope_id}>
                <strong>{options.find(o => o.kind === 'tenant' && o.id === m.scope_id)?.label || m.scope_id}</strong>
                <button type="button" className="btn btn-sm btn-secondary" onClick={() => setInspecting(m.scope_id)}>{t('users.inspectAccess')}</button>
              </li>
            ))}
          </ul>
          {inspecting && <TenantAccessInspection key={inspecting} tenantId={inspecting} userId={user.id}
            onClose={() => setInspecting(null)} />}
        </section>
      )}
    </div>
  );
}

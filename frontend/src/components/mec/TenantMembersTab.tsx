import DelegatedManagedGrant from './DelegatedManagedGrant';
import MembershipOperations from './MembershipOperations';
import ManagedMembershipPolicies from './ManagedMembershipPolicies';
import { useApi } from '../../shared/api/useApi';
import InitialCredentials, { BootstrapCredentials } from './InitialCredentials';
import React, { useState } from 'react';
import { useT } from '../../i18n';
import { useMecList, mecPost, mecDelete } from '../../hooks/mec/useMecApi';
import {
  ErrorBanner,
  Section,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';

interface Member {
  id: string;
  user_id: string;
  username?: string | null;
  display_name?: string | null;
  role_template: string;
}

interface Props {
  tenantId: string;
}

const ROLES = [
  { value: 'project-owner', labelKey: 'mec.tenant.members.roleOwner' },
  { value: 'project-member', labelKey: 'mec.tenant.members.roleMember' },
  { value: 'read-only', labelKey: 'mec.tenant.members.roleReadOnly' },
];

export default function TenantMembersTab({ tenantId }: Props) {
  const { t } = useT();
  const { data: me } = useApi<{ role: string; status: string; memberships: { kind: string; scope_id: string; role: string; enabled: boolean }[] }>('/api/users/me');
  const canManage = me?.role === 'admin' && me.status === 'active';
  const isTenantManager = me?.status === 'active' && me.memberships?.some(m => m.kind === 'tenant' && m.scope_id === tenantId && m.role === 'admin' && m.enabled);
  const [operationRevision, setOperationRevision] = useState(0);
  const { data, loading, error, refetch } = useMecList<Member>(
    canManage ? `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members` : '',
    60_000,
  );
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState('project-member');
  const [createIfMissing, setCreateIfMissing] = useState(false);
  const [credentials, setCredentials] = useState<BootstrapCredentials | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost<{ initial_credentials?: BootstrapCredentials | null }>(
        `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members`,
        {
          username,
          password: password || undefined,
          role,
          create_if_missing: createIfMissing,
        },
      );
      if (res.error) setErr(res.error.message);
      else {
        setCredentials(res.data?.initial_credentials || null);
        setUsername('');
        setPassword('');
        await refetch();
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setOperationRevision(value => value + 1);
      setBusy(false);
    }
  };

  const remove = async (id: string, user: string) => {
    if (!(await confirmDialog({ message: t('mec.tenant.members.removeConfirm', { user }), danger: true, confirmLabel: t('common.delete') }))) return;
    try {
      await mecDelete(
        `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members/${encodeURIComponent(id)}`,
      );
      await refetch();
    } catch (e) {
      setErr(String(e));
    } finally { setOperationRevision(value => value + 1); }
  };

  return (
    <div>
      <div
        style={{
          display: 'flex',
          justifyContent: 'flex-end',
          marginBottom: '8px',
        }}
      >
        <button className="btn btn-secondary btn-small" onClick={refetch}>
          {t('mec.action.refresh')}
        </button>
      </div>
      <ErrorBanner error={error || err || undefined} />

      {credentials && <InitialCredentials credentials={credentials} onDone={() => setCredentials(null)} />}

      {canManage && <details><summary>{t('mec.managed.legacyAdd')}</summary><p>{t('mec.managed.legacyWarning')}</p><Section title={t('mec.tenant.members.add')}>
        <form
          onSubmit={add}
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            gap: '8px',
            alignItems: 'flex-end',
          }}
        >
          <label style={{ flex: '1 1 180px' }}>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
              {t('mec.tenant.members.username')}
            </div>
            <input
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="ygram-dev"
              style={{ width: '100%' }}
            />
          </label>
          <label style={{ flex: '1 1 160px' }}>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
              {t('mec.tenant.members.initialPassword')}
            </div>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('mec.credentials.generatedIfBlank')}
              minLength={15}
              disabled={!createIfMissing}
              style={{ width: '100%' }}
            />
          </label>
          <label style={{ flex: '1 1 160px' }}>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
              {t('mec.tenant.members.role')}
            </div>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              style={{ width: '100%' }}
            >
              {ROLES.map((r) => (
                <option key={r.value} value={r.value}>
                  {t(r.labelKey)}
                </option>
              ))}
            </select>
          </label>
          <label style={{ marginBottom: '8px' }}>
            <input
              type="checkbox"
              checked={createIfMissing}
              onChange={(e) => setCreateIfMissing(e.target.checked)}
            />{' '}
            {t('mec.tenant.members.createIfMissing')}
          </label>
          <button type="submit" className="btn btn-primary" disabled={busy}>
            {busy ? t('mec.tenant.members.adding') : t('mec.tenant.members.addBtn')}
          </button>
        </form>
      </Section></details>}

      <Section title={t('mec.tenant.members.list')} marginTop="20px">
        <Toolbar>
          <input
            placeholder={t('mec.tenant.members.filter')}
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            style={{ padding: '6px 10px', minWidth: '240px' }}
          />
        </Toolbar>
        {loading && !data && (
          <div style={{ color: 'var(--text-secondary)' }}>{t('mec.state.loading')}</div>
        )}
        <SortableTable<Member>
          data={data || []}
          filter={filter}
          defaultSortKey="username"
          rowKey={(m) => m.id}
          emptyMessage={t('mec.tenant.members.empty')}
          columns={[
            {
              key: 'username',
              header: t('mec.tenant.members.colUsername'),
              accessor: (m) => m.username || m.user_id,
              render: (m) => <code>{m.username || m.user_id}</code>,
              sortable: true,
            },
            {
              key: 'display_name',
              header: t('mec.tenant.members.colDisplayName'),
              accessor: (m) => m.display_name || '',
              render: (m) =>
                m.display_name || (
                  <span style={{ color: 'var(--text-secondary)' }}>-</span>
                ),
              sortable: true,
            },
            {
              key: 'role',
              header: t('mec.tenant.members.colRole'),
              accessor: (m) => m.role_template,
              render: (m) => (
                <StatusBadge tone="info">{m.role_template}</StatusBadge>
              ),
              sortable: true,
            },
            {
              key: 'binding',
              header: t('mec.tenant.members.colBindingId'),
              accessor: (m) => m.id,
              render: (m) => (
                <code style={{ fontSize: '11px' }}>{m.id}</code>
              ),
              sortable: true,
            },
            {
              key: 'actions',
              header: '',
              accessor: () => '',
              sortable: false,
              width: '80px',
              render: (m) => canManage ? (
                <button
                  className="btn btn-danger btn-small"
                  onClick={() => remove(m.id, m.username || m.user_id)}
                >
                  {t('mec.tenant.members.remove')}
                </button>
              ) : null,
            },
          ]}
        />
      </Section>
      {!canManage && isTenantManager && <DelegatedManagedGrant tenantId={tenantId} />}
      {canManage && <><MembershipOperations tenantId={tenantId} revision={operationRevision} /><ManagedMembershipPolicies tenantId={tenantId} onChanged={async () => { setOperationRevision(value => value + 1); await refetch(); }} /></>}
    </div>
  );
}

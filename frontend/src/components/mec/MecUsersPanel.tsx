import React, { useState } from 'react';
import { useT } from '../../i18n';
import {
  useMecList,
  mecPost,
  mecDelete,
} from '../../hooks/mec/useMecApi';
import {
  ErrorBanner,
  PanelLayout,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

interface RancherUser {
  id: string;
  username: string;
  display_name?: string | null;
  enabled: boolean;
  must_change_password: boolean;
}

function randomPassword(len = 16): string {
  const chars =
    'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789!@#$';
  let out = '';
  for (let i = 0; i < len; i++) {
    out += chars[Math.floor(Math.random() * chars.length)];
  }
  return out;
}

export default function MecUsersPanel() {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecList<RancherUser>(
    '/api/mec/v1/users',
    60_000,
  );
  const [showAdd, setShowAdd] = useState(false);
  const [form, setForm] = useState({ username: '', password: randomPassword() });
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost('/api/mec/v1/users', form);
      if (res.error) setErr(res.error.message);
      else {
        setToast(
          t('mec.users.createdToast', { username: form.username, password: form.password }),
        );
        setShowAdd(false);
        setForm({ username: '', password: randomPassword() });
        await refetch();
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (u: RancherUser) => {
    const typed = window.prompt(
      t('mec.users.deletePrompt', { username: u.username, id: u.id }),
      '',
    );
    if (typed !== u.id) {
      if (typed !== null) setErr(t('mec.users.deleteMismatch'));
      return;
    }
    try {
      await mecDelete(`/api/mec/v1/users/${encodeURIComponent(u.id)}`, u.id);
      await refetch();
    } catch (e) {
      setErr(String(e));
    }
  };

  const resetPassword = async (u: RancherUser) => {
    const newPw = window.prompt(
      t('mec.users.resetPrompt', { username: u.username }),
      randomPassword(),
    );
    if (newPw === null) return;
    const final = newPw || randomPassword();
    try {
      await fetch(
        `/api/mec/v1/users/${encodeURIComponent(u.id)}/reset-password`,
        {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${sessionStorage.getItem('nabiman_token') || ''}`,
            'X-Confirm-Name': u.id,
          },
          body: JSON.stringify({ new_password: final }),
        },
      );
      setToast(t('mec.users.resetToast', { username: u.username, password: final }));
    } catch (e) {
      setErr(String(e));
    }
  };

  const users = data || [];
  const enabled = users.filter((u) => u.enabled).length;

  return (
    <PanelLayout
      title={t('mec.users.title')}
      subtitle={t('mec.users.subtitle', { total: users.length, enabled })}
      actions={
        <>
          <button className="btn btn-secondary" onClick={refetch}>
            {t('mec.action.refresh')}
          </button>
          <button className="btn btn-primary" onClick={() => setShowAdd((v) => !v)}>
            {showAdd ? t('mec.users.cancel') : t('mec.users.addUser')}
          </button>
        </>
      }
    >
      <ErrorBanner error={error || err || undefined} />

      {toast && (
        <div
          style={{
            background: '#fef3c7',
            borderLeft: '3px solid #f59e0b',
            padding: '10px 14px',
            borderRadius: '6px',
            fontSize: '13px',
            marginBottom: '12px',
          }}
        >
          <strong>🔐 </strong>{toast}
          <button
            style={{ float: 'right' }}
            className="btn btn-secondary btn-small"
            onClick={() => setToast(null)}
          >
            {t('mec.users.close')}
          </button>
        </div>
      )}

      {showAdd && (
        <form
          onSubmit={create}
          style={{
            background: 'white',
            border: '1px solid #e5e7eb',
            padding: '14px',
            borderRadius: '8px',
            marginBottom: '12px',
          }}
        >
          <h3 style={{ marginTop: 0 }}>{t('mec.users.createTitle')}</h3>
          <div
            style={{
              display: 'flex',
              gap: '8px',
              alignItems: 'flex-end',
              flexWrap: 'wrap',
            }}
          >
            <label style={{ flex: '1 1 160px' }}>
              <div style={{ fontSize: '11px', color: '#6b7280' }}>Username</div>
              <input
                required
                value={form.username}
                onChange={(e) => setForm({ ...form, username: e.target.value })}
                placeholder="ygram-dev"
                style={{ width: '100%' }}
              />
            </label>
            <label style={{ flex: '1 1 200px' }}>
              <div style={{ fontSize: '11px', color: '#6b7280' }}>
                {t('mec.users.initialPassword')}
              </div>
              <input
                required
                value={form.password}
                onChange={(e) => setForm({ ...form, password: e.target.value })}
                style={{ width: '100%' }}
              />
            </label>
            <button
              type="button"
              className="btn btn-secondary btn-small"
              onClick={() => setForm({ ...form, password: randomPassword() })}
            >
              {t('mec.users.auto')}
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {busy ? t('mec.users.creating') : t('mec.users.create')}
            </button>
          </div>
        </form>
      )}

      <Toolbar>
        <input
          placeholder={t('mec.users.filterPlaceholder')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      <SortableTable<RancherUser>
        data={users}
        filter={filter}
        defaultSortKey="username"
        rowKey={(u) => u.id}
        emptyMessage={t('mec.users.empty')}
        columns={[
          {
            key: 'id',
            header: 'ID',
            accessor: (u) => u.id,
            render: (u) => <code style={{ fontSize: '11px' }}>{u.id}</code>,
            sortable: true,
          },
          {
            key: 'username',
            header: 'Username',
            accessor: (u) => u.username,
            render: (u) => <strong>{u.username}</strong>,
            sortable: true,
          },
          {
            key: 'display_name',
            header: t('mec.users.colDisplayName'),
            accessor: (u) => u.display_name || '',
            render: (u) =>
              u.display_name || <span style={{ color: '#d1d5db' }}>-</span>,
            sortable: true,
          },
          {
            key: 'enabled',
            header: t('mec.users.colEnabled'),
            accessor: (u) => u.enabled,
            render: (u) => (
              <StatusBadge tone={u.enabled}>
                {u.enabled ? 'active' : 'disabled'}
              </StatusBadge>
            ),
            width: '100px',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '200px',
            render: (u) => (
              <div style={{ display: 'flex', gap: '4px' }}>
                <button
                  className="btn btn-secondary btn-small"
                  onClick={() => resetPassword(u)}
                >
                  {t('mec.users.resetPassword')}
                </button>
                <button
                  className="btn btn-danger btn-small"
                  onClick={() => remove(u)}
                >
                  {t('mec.users.delete')}
                </button>
              </div>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}

import React, { useState } from 'react';
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
          `사용자 ${form.username} 생성됨. 초기 비밀번호: ${form.password} (한 번만 표시)`,
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
      `⚠️ 사용자 '${u.username}' (${u.id}) 을(를) 삭제합니다.\n확인을 위해 ID 를 다시 입력하세요:`,
      '',
    );
    if (typed !== u.id) {
      if (typed !== null) setErr('입력 ID 가 일치하지 않아 삭제 취소.');
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
      `${u.username} 의 새 비밀번호 (비우면 자동 생성):`,
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
      setToast(`${u.username} 비밀번호 리셋: ${final} (한 번만 표시)`);
    } catch (e) {
      setErr(String(e));
    }
  };

  const users = data || [];
  const enabled = users.filter((u) => u.enabled).length;

  return (
    <PanelLayout
      title="Rancher 사용자"
      subtitle={`${users.length}명 · 활성 ${enabled}`}
      actions={
        <>
          <button className="btn btn-secondary" onClick={refetch}>
            새로고침
          </button>
          <button className="btn btn-primary" onClick={() => setShowAdd((v) => !v)}>
            {showAdd ? '취소' : '+ 사용자'}
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
            닫기
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
          <h3 style={{ marginTop: 0 }}>사용자 생성</h3>
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
                초기 비밀번호
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
              🎲 자동
            </button>
            <button type="submit" className="btn btn-primary" disabled={busy}>
              {busy ? '생성 중...' : '생성'}
            </button>
          </div>
        </form>
      )}

      <Toolbar>
        <input
          placeholder="필터 (ID, username)"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>로딩 중...</div>}

      <SortableTable<RancherUser>
        data={users}
        filter={filter}
        defaultSortKey="username"
        rowKey={(u) => u.id}
        emptyMessage="사용자가 없습니다."
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
            header: '표시명',
            accessor: (u) => u.display_name || '',
            render: (u) =>
              u.display_name || <span style={{ color: '#d1d5db' }}>-</span>,
            sortable: true,
          },
          {
            key: 'enabled',
            header: '활성',
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
                  비번 리셋
                </button>
                <button
                  className="btn btn-danger btn-small"
                  onClick={() => remove(u)}
                >
                  삭제
                </button>
              </div>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}

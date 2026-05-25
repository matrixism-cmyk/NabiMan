import React, { useState } from 'react';
import { useMecList, mecPost, mecDelete } from '../../hooks/mec/useMecApi';
import {
  ErrorBanner,
  Section,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

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
  { value: 'project-owner', label: 'Owner (전체 관리)' },
  { value: 'project-member', label: 'Member (읽기/쓰기)' },
  { value: 'read-only', label: 'Read-only (조회만)' },
];

export default function TenantMembersTab({ tenantId }: Props) {
  const { data, loading, error, refetch } = useMecList<Member>(
    `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members`,
    60_000,
  );
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [role, setRole] = useState('project-owner');
  const [createIfMissing, setCreateIfMissing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const add = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost(
        `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members`,
        {
          username,
          password: password || 'changeme!',
          role,
          create_if_missing: createIfMissing,
        },
      );
      if (res.error) setErr(res.error.message);
      else {
        setUsername('');
        setPassword('');
        await refetch();
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string, user: string) => {
    if (!window.confirm(`멤버 ${user} 제거?`)) return;
    try {
      await mecDelete(
        `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/members/${encodeURIComponent(id)}`,
      );
      await refetch();
    } catch (e) {
      setErr(String(e));
    }
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
          새로고침
        </button>
      </div>
      <ErrorBanner error={error || err || undefined} />

      <Section title="멤버 추가">
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
              사용자명
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
              초기 비밀번호 (신규 생성 시)
            </div>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="changeme!"
              style={{ width: '100%' }}
            />
          </label>
          <label style={{ flex: '1 1 160px' }}>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>
              권한
            </div>
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              style={{ width: '100%' }}
            >
              {ROLES.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
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
            사용자 없으면 생성
          </label>
          <button type="submit" className="btn btn-primary" disabled={busy}>
            {busy ? '추가 중...' : '+ 추가'}
          </button>
        </form>
      </Section>

      <Section title="멤버 목록" marginTop="20px">
        <Toolbar>
          <input
            placeholder="필터 (username, role)"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            style={{ padding: '6px 10px', minWidth: '240px' }}
          />
        </Toolbar>
        {loading && !data && (
          <div style={{ color: 'var(--text-secondary)' }}>로딩 중...</div>
        )}
        <SortableTable<Member>
          data={data || []}
          filter={filter}
          defaultSortKey="username"
          rowKey={(m) => m.id}
          emptyMessage="이 테넌트에 멤버가 없습니다."
          columns={[
            {
              key: 'username',
              header: '사용자명',
              accessor: (m) => m.username || m.user_id,
              render: (m) => <code>{m.username || m.user_id}</code>,
              sortable: true,
            },
            {
              key: 'display_name',
              header: '표시명',
              accessor: (m) => m.display_name || '',
              render: (m) =>
                m.display_name || (
                  <span style={{ color: 'var(--text-secondary)' }}>-</span>
                ),
              sortable: true,
            },
            {
              key: 'role',
              header: '권한',
              accessor: (m) => m.role_template,
              render: (m) => (
                <StatusBadge tone="info">{m.role_template}</StatusBadge>
              ),
              sortable: true,
            },
            {
              key: 'binding',
              header: '바인딩 ID',
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
              render: (m) => (
                <button
                  className="btn btn-danger btn-small"
                  onClick={() => remove(m.id, m.username || m.user_id)}
                >
                  제거
                </button>
              ),
            },
          ]}
        />
      </Section>
    </div>
  );
}

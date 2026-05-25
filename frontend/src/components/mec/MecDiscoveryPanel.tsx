import React, { useState } from 'react';
import { useMecList, mecPost } from '../../hooks/mec/useMecApi';
import {
  ErrorBanner,
  PanelLayout,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

interface DiscoveredTenant {
  id: string;
  namespace: string;
  labels: Record<string, string>;
  already_managed: boolean;
  suspected_tenant: boolean;
}

export default function MecDiscoveryPanel() {
  const { data, loading, error, refetch } = useMecList<DiscoveredTenant>(
    '/api/mec/v1/tenants/discover',
    60_000,
  );
  const [busyId, setBusyId] = useState<string | null>(null);
  const [actionErr, setActionErr] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const importOne = async (id: string) => {
    setBusyId(id);
    setActionErr(null);
    try {
      const res = await mecPost(
        `/api/mec/v1/tenants/${encodeURIComponent(id)}/import`,
        {},
      );
      if (res.error) setActionErr(res.error.message);
      else await refetch();
    } catch (e) {
      setActionErr(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const rows = data || [];
  const unmanaged = rows.filter((r) => !r.already_managed).length;

  return (
    <PanelLayout
      title="테넌트 Discovery"
      subtitle={`발견 ${rows.length} · 미등록 ${unmanaged} · Import 로 기존 리소스를 재생성 없이 등록`}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          다시 스캔
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />
      <ErrorBanner error={actionErr || undefined} />

      <div
        style={{
          background: '#f0f9ff',
          borderLeft: '3px solid #3b82f6',
          color: '#1e40af',
          padding: '10px 14px',
          borderRadius: '6px',
          fontSize: '13px',
          marginBottom: '12px',
        }}
      >
        ℹ️ <strong>Discovery</strong>는 클러스터에서 <code>-poc/-dev/-prod</code>{' '}
        접미사 또는 <code>tenant</code> 라벨이 있는 네임스페이스를 자동 탐색합니다.
        Import 시 기존 K8s / Rancher 리소스를 건드리지 않고 NabiMan DB 에만
        등록합니다.
      </div>

      <Toolbar>
        <input
          placeholder="필터 (네임스페이스)"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>스캔 중...</div>}

      <SortableTable<DiscoveredTenant>
        data={rows}
        filter={filter}
        defaultSortKey="namespace"
        rowKey={(r) => r.id}
        emptyMessage="발견된 미관리 테넌트가 없습니다."
        columns={[
          {
            key: 'namespace',
            header: '네임스페이스',
            accessor: (r) => r.namespace,
            render: (r) => <code>{r.namespace}</code>,
            sortable: true,
          },
          {
            key: 'labels',
            header: 'Labels (일부)',
            accessor: (r) =>
              Object.entries(r.labels)
                .slice(0, 4)
                .map(([k, v]) => `${k}=${v}`)
                .join(','),
            render: (r) => (
              <span style={{ fontSize: '11px', fontFamily: 'monospace' }}>
                {Object.entries(r.labels)
                  .filter(
                    ([k]) =>
                      !k.startsWith('kubernetes.io/') &&
                      !k.startsWith('app.kubernetes.io/'),
                  )
                  .slice(0, 4)
                  .map(([k, v]) => `${k}=${v}`)
                  .join(', ') || <span style={{ color: '#d1d5db' }}>-</span>}
              </span>
            ),
            sortable: true,
          },
          {
            key: 'status',
            header: '상태',
            accessor: (r) => (r.already_managed ? 'imported' : 'unmanaged'),
            render: (r) => (
              <StatusBadge tone={r.already_managed ? 'success' : 'warning'}>
                {r.already_managed ? 'imported' : 'unmanaged'}
              </StatusBadge>
            ),
            width: '120px',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '120px',
            render: (r) =>
              r.already_managed ? (
                <span style={{ color: '#9ca3af', fontSize: '12px' }}>
                  등록됨
                </span>
              ) : (
                <button
                  className="btn btn-primary btn-small"
                  disabled={busyId === r.id}
                  onClick={() => importOne(r.id)}
                >
                  {busyId === r.id ? 'Import 중...' : 'Import'}
                </button>
              ),
          },
        ]}
      />
    </PanelLayout>
  );
}

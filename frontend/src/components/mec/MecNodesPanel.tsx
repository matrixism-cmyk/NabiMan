import React, { useState } from 'react';
import { useMecList } from '../../hooks/mec/useMecApi';
import { MecNode } from '../../types/mec';
import MecNodeDetail from './MecNodeDetail';
import {
  BarChart,
  ErrorBanner,
  PanelLayout,
  Section,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

function parseMemoryGi(s: string): number {
  if (!s) return 0;
  const match = /^([\d.]+)\s*(Ki|Mi|Gi|Ti|Pi|K|M|G|T)?$/.exec(s.trim());
  if (!match) return parseFloat(s) || 0;
  const v = parseFloat(match[1]);
  const unit = match[2] || 'Gi';
  switch (unit) {
    case 'Ki':
    case 'K':
      return v / (1024 * 1024);
    case 'Mi':
    case 'M':
      return v / 1024;
    case 'Gi':
    case 'G':
      return v;
    case 'Ti':
    case 'T':
      return v * 1024;
    case 'Pi':
      return v * 1024 * 1024;
    default:
      return v;
  }
}

export default function MecNodesPanel() {
  const { data, loading, error, refetch } = useMecList<MecNode>(
    '/api/mec/v1/nodes',
    30_000,
  );
  const [selected, setSelected] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  if (selected) {
    return (
      <MecNodeDetail
        nodeName={selected}
        onClose={() => {
          setSelected(null);
          refetch();
        }}
      />
    );
  }

  const nodes = data || [];
  const ready = nodes.filter((n) => n.status === 'Ready').length;
  const totalGpu = nodes.reduce((s, n) => s + (n.gpu_info?.total_slots || 0), 0);
  const allocatedGpu = nodes
    .filter((n) => n.current_tenant)
    .reduce((s, n) => s + (n.gpu_info?.total_slots || 0), 0);

  return (
    <PanelLayout
      title="MEC 노드"
      subtitle={`${nodes.length}개 노드 · Ready ${ready} · GPU ${allocatedGpu}/${totalGpu} slots`}
      actions={
        <button className="btn btn-secondary" onClick={refetch}>
          새로고침
        </button>
      }
    >
      <ErrorBanner error={error || undefined} />

      {nodes.length > 0 && totalGpu > 0 && (
        <Section title="노드별 GPU slot (할당 vs 전체)">
          <BarChart
            data={nodes
              .filter((n) => (n.gpu_info?.total_slots ?? 0) > 0)
              .map((n) => ({
                label: `${n.name} — ${n.gpu_info?.model || ''}`,
                value: n.current_tenant ? n.gpu_info!.total_slots : 0,
                max: n.gpu_info!.total_slots,
                helper: n.current_tenant
                  ? `tenant: ${n.current_tenant}`
                  : '미할당',
              }))}
            showValue
          />
        </Section>
      )}

      <Toolbar marginBottom="12px">
        <input
          placeholder="필터 (이름/GPU/tenant)"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ padding: '6px 10px', minWidth: '240px' }}
        />
      </Toolbar>

      {loading && !data && <div style={{ color: '#6b7280' }}>로딩 중...</div>}

      <SortableTable<MecNode>
        data={nodes}
        filter={filter}
        defaultSortKey="name"
        rowKey={(n) => n.name}
        onRowClick={(n) => setSelected(n.name)}
        emptyMessage="노드가 없습니다."
        columns={[
          {
            key: 'name',
            header: '노드',
            accessor: (n) => n.name,
            render: (n) => <code>{n.name}</code>,
            sortable: true,
          },
          {
            key: 'status',
            header: '상태',
            accessor: (n) => n.status,
            render: (n) => <StatusBadge tone={n.status}>{n.status}</StatusBadge>,
            width: '100px',
            sortable: true,
          },
          {
            key: 'roles',
            header: '역할',
            accessor: (n) => n.roles.join(','),
            render: (n) => (
              <span style={{ color: '#6b7280' }}>
                {n.roles.join(', ') || 'worker'}
              </span>
            ),
            sortable: true,
          },
          {
            key: 'arch',
            header: 'Arch',
            accessor: (n) => n.architecture,
            sortable: true,
          },
          {
            key: 'cpu',
            header: 'CPU',
            accessor: (n) => parseFloat(n.capacity.cpu) || 0,
            render: (n) => n.capacity.cpu,
            align: 'right',
            sortable: true,
          },
          {
            key: 'mem',
            header: 'Memory',
            accessor: (n) => parseMemoryGi(n.capacity.memory),
            render: (n) => n.capacity.memory,
            align: 'right',
            sortable: true,
          },
          {
            key: 'gpu',
            header: 'GPU',
            accessor: (n) =>
              n.gpu_info ? `${n.gpu_info.model} ${n.gpu_info.total_slots}` : '',
            render: (n) =>
              n.gpu_info ? (
                <span>
                  <span style={{ color: '#6b7280' }}>{n.gpu_info.model}</span>
                  <span style={{ margin: '0 4px', color: '#d1d5db' }}>·</span>
                  <span style={{ fontWeight: 600 }}>
                    {n.gpu_info.count}×({n.gpu_info.total_slots})
                  </span>
                </span>
              ) : (
                <span style={{ color: '#d1d5db' }}>-</span>
              ),
            sortable: true,
          },
          {
            key: 'mode',
            header: 'Mode',
            accessor: (n) => n.gpu_info?.mode || '',
            render: (n) =>
              n.gpu_info ? (
                <StatusBadge tone="info">{n.gpu_info.mode}</StatusBadge>
              ) : (
                <span style={{ color: '#d1d5db' }}>-</span>
              ),
            sortable: true,
          },
          {
            key: 'tenant',
            header: 'Tenant',
            accessor: (n) => n.current_tenant || '',
            render: (n) =>
              n.current_tenant ? (
                <StatusBadge tone="info">{n.current_tenant}</StatusBadge>
              ) : (
                <span style={{ color: '#9ca3af' }}>미할당</span>
              ),
            sortable: true,
          },
          {
            key: 'taints',
            header: 'Taints',
            accessor: (n) => n.taints.length,
            render: (n) =>
              n.taints.length > 0 ? (
                <span
                  title={n.taints
                    .map(
                      (t) =>
                        `${t.key}${t.value ? '=' + t.value : ''}:${t.effect}`,
                    )
                    .join('\n')}
                >
                  {n.taints.length}
                </span>
              ) : (
                <span style={{ color: '#d1d5db' }}>0</span>
              ),
            align: 'right',
            width: '70px',
            sortable: true,
          },
          {
            key: 'actions',
            header: '',
            accessor: () => '',
            sortable: false,
            width: '80px',
            render: (n) => (
              <button
                className="btn btn-secondary btn-small"
                onClick={(e) => {
                  e.stopPropagation();
                  setSelected(n.name);
                }}
              >
                상세
              </button>
            ),
          },
        ]}
      />
    </PanelLayout>
  );
}

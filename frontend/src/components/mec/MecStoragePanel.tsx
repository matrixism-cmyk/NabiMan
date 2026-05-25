import React, { useState } from 'react';
import { useMecList, mecPost } from '../../hooks/mec/useMecApi';
import {
  BarChart,
  ErrorBanner,
  MetricCard,
  MetricGrid,
  PanelLayout,
  Section,
  SortableTable,
  StatusBadge,
  Toolbar,
} from './common';

interface HarborProject {
  id: number;
  name: string;
  public: boolean;
  repo_count: number;
  created_at: string;
}

interface LbPool {
  name: string;
  address_ranges: string[];
  total_ips: number;
  used_ips: number;
  auto_assign: boolean;
}

interface Pvc {
  namespace: string;
  name: string;
  phase: string;
  storage_request: string;
  storage_class?: string | null;
  access_modes: string[];
  age_seconds: number;
}

function humanizeSec(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

export default function MecStoragePanel() {
  const pools = useMecList<LbPool>('/api/mec/v1/network/lb-pools', 30_000);
  const pvcs = useMecList<Pvc>('/api/mec/v1/storage/pvcs', 60_000);
  const harbor = useMecList<HarborProject>(
    '/api/mec/v1/storage/harbor/projects',
    60_000,
  );
  const [newProject, setNewProject] = useState('');
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [pvcFilter, setPvcFilter] = useState('');

  const refreshAll = () => {
    pools.refetch();
    harbor.refetch();
    pvcs.refetch();
  };

  const createProject = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newProject) return;
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost('/api/mec/v1/storage/harbor/projects', {
        name: newProject,
        public: false,
      });
      if (res.error) setErr(res.error.message);
      else {
        setNewProject('');
        await harbor.refetch();
      }
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const poolData = pools.data || [];
  const pvcData = pvcs.data || [];
  const harborData = harbor.data || [];

  const totalIps = poolData.reduce((s, p) => s + p.total_ips, 0);
  const usedIps = poolData.reduce((s, p) => s + p.used_ips, 0);
  const boundPvcs = pvcData.filter((p) => p.phase === 'Bound').length;

  return (
    <PanelLayout
      title="스토리지 / LB Pool / Harbor"
      subtitle={`LB ${usedIps}/${totalIps} · PVC ${pvcData.length}개 (Bound ${boundPvcs}) · Harbor ${harborData.length}개 프로젝트`}
      actions={
        <button className="btn btn-secondary" onClick={refreshAll}>
          새로고침
        </button>
      }
    >
      <ErrorBanner error={err || undefined} />

      <MetricGrid>
        <MetricCard
          label="LB IP 사용"
          value={usedIps}
          unit={`/ ${totalIps}`}
          helper={`${poolData.length}개 Pool`}
          tone={usedIps / Math.max(1, totalIps) > 0.9 ? 'error' : 'info'}
        />
        <MetricCard
          label="PVC"
          value={pvcData.length}
          helper={`Bound ${boundPvcs}`}
          tone="info"
        />
        <MetricCard
          label="Harbor 프로젝트"
          value={harborData.length}
          tone="info"
        />
      </MetricGrid>

      <Section title="MetalLB IP Pool" marginTop="24px">
        {pools.error && (
          <div style={{ color: 'var(--danger)', fontSize: '13px' }}>
            {pools.error}
          </div>
        )}
        {poolData.length > 0 ? (
          <BarChart
            data={poolData.map((p) => ({
              label: `${p.name} — ${p.address_ranges.join(', ')}`,
              value: p.used_ips,
              max: p.total_ips,
              helper: p.auto_assign ? 'auto-assign' : 'manual',
            }))}
            showValue
          />
        ) : (
          !pools.loading && (
            <div
              style={{ color: 'var(--text-secondary)', fontStyle: 'italic' }}
            >
              Pool 이 없습니다.
            </div>
          )
        )}
      </Section>

      <Section title="PersistentVolumeClaim" marginTop="24px">
        <Toolbar>
          <input
            placeholder="필터 (ns, name, storage-class)"
            value={pvcFilter}
            onChange={(e) => setPvcFilter(e.target.value)}
            style={{ padding: '6px 10px', minWidth: '240px' }}
          />
        </Toolbar>
        <SortableTable<Pvc>
          data={pvcData}
          filter={pvcFilter}
          defaultSortKey="namespace"
          rowKey={(p) => `${p.namespace}/${p.name}`}
          emptyMessage="PVC 가 없습니다."
          columns={[
            {
              key: 'namespace',
              header: 'Namespace',
              accessor: (p) => p.namespace,
              sortable: true,
            },
            {
              key: 'name',
              header: '이름',
              accessor: (p) => p.name,
              render: (p) => <code>{p.name}</code>,
              sortable: true,
            },
            {
              key: 'phase',
              header: '상태',
              accessor: (p) => p.phase,
              render: (p) => <StatusBadge tone={p.phase}>{p.phase}</StatusBadge>,
              width: '100px',
              sortable: true,
            },
            {
              key: 'size',
              header: '요청',
              accessor: (p) => p.storage_request,
              align: 'right',
              sortable: true,
            },
            {
              key: 'sc',
              header: 'StorageClass',
              accessor: (p) => p.storage_class || '',
              render: (p) =>
                p.storage_class || (
                  <span style={{ color: 'var(--text-secondary)' }}>-</span>
                ),
              sortable: true,
            },
            {
              key: 'modes',
              header: 'Access',
              accessor: (p) => p.access_modes.join(','),
              render: (p) => p.access_modes.join(', ') || '-',
              sortable: true,
            },
            {
              key: 'age',
              header: 'Age',
              accessor: (p) => p.age_seconds,
              render: (p) => humanizeSec(p.age_seconds),
              width: '80px',
              align: 'right',
              sortable: true,
            },
          ]}
        />
      </Section>

      <Section title="Harbor 프로젝트" marginTop="24px">
        <form
          onSubmit={createProject}
          style={{ display: 'flex', gap: '8px', marginBottom: '12px' }}
        >
          <input
            value={newProject}
            onChange={(e) => setNewProject(e.target.value)}
            placeholder="프로젝트 이름 (예: ygram-poc)"
            style={{ flex: 1, padding: '6px 10px' }}
          />
          <button
            type="submit"
            className="btn btn-primary"
            disabled={busy || !newProject}
          >
            {busy ? '생성 중...' : '+ 프로젝트'}
          </button>
        </form>
        <SortableTable<HarborProject>
          data={harborData}
          defaultSortKey="id"
          rowKey={(p) => p.id}
          emptyMessage="Harbor 프로젝트가 없습니다."
          columns={[
            {
              key: 'id',
              header: 'ID',
              accessor: (p) => p.id,
              align: 'right',
              width: '60px',
              sortable: true,
            },
            {
              key: 'name',
              header: '이름',
              accessor: (p) => p.name,
              render: (p) => <code>{p.name}</code>,
              sortable: true,
            },
            {
              key: 'public',
              header: '공개',
              accessor: (p) => p.public,
              render: (p) => (
                <StatusBadge tone={p.public ? 'info' : 'neutral'}>
                  {p.public ? 'public' : 'private'}
                </StatusBadge>
              ),
              width: '100px',
              sortable: true,
            },
            {
              key: 'repos',
              header: '저장소',
              accessor: (p) => p.repo_count,
              align: 'right',
              width: '80px',
              sortable: true,
            },
            {
              key: 'created',
              header: '생성',
              accessor: (p) => p.created_at,
              render: (p) => new Date(p.created_at).toLocaleDateString(),
              width: '120px',
              sortable: true,
            },
          ]}
        />
      </Section>
    </PanelLayout>
  );
}

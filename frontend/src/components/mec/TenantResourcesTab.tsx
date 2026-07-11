import React from 'react';
import { useT } from '../../i18n';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { ErrorBanner, Section, SortableTable, StatusBadge } from './common';

interface Props {
  tenantId: string;
}

interface PodRow {
  namespace: string;
  name: string;
  phase: string;
  node_name?: string | null;
  cpu_requests: string;
  memory_requests: string;
  gpu_requests: number;
  ready_containers: number;
  containers: number;
}

interface ServicePort {
  port: number;
  target_port: number;
  protocol: string;
}

interface ServiceRow {
  namespace: string;
  name: string;
  external_ip: string;
  ports: ServicePort[];
  selector_summary: string;
  age_seconds: number;
}

interface ResourcesResponse {
  pods: PodRow[];
  services: ServiceRow[];
}

function humanize(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

export default function TenantResourcesTab({ tenantId }: Props) {
  const { t } = useT();
  const { data, loading, error, refetch } = useMecApi<ResourcesResponse>(
    `/api/mec/v1/tenants/${encodeURIComponent(tenantId)}/resources`,
    15_000,
  );

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
      {loading && !data && (
        <div style={{ color: 'var(--text-secondary)' }}>{t('mec.state.loading')}</div>
      )}
      <ErrorBanner error={error || undefined} />

      <Section title="Pods">
        <SortableTable<PodRow>
          data={data?.pods || []}
          defaultSortKey="name"
          rowKey={(p) => p.name}
          emptyMessage={t('mec.tenant.resources.podsEmpty')}
          columns={[
            {
              key: 'name',
              header: t('mec.tenant.resources.colName'),
              accessor: (p) => p.name,
              render: (p) => <code>{p.name}</code>,
              sortable: true,
            },
            {
              key: 'phase',
              header: 'Phase',
              accessor: (p) => p.phase,
              render: (p) => <StatusBadge tone={p.phase}>{p.phase}</StatusBadge>,
              width: '110px',
              sortable: true,
            },
            {
              key: 'node',
              header: t('mec.tenant.resources.colNode'),
              accessor: (p) => p.node_name || '',
              render: (p) =>
                p.node_name || (
                  <span style={{ color: 'var(--text-secondary)' }}>-</span>
                ),
              sortable: true,
            },
            {
              key: 'cpu',
              header: 'CPU req',
              accessor: (p) => parseFloat(p.cpu_requests) || 0,
              render: (p) => p.cpu_requests || '-',
              align: 'right',
              sortable: true,
            },
            {
              key: 'mem',
              header: 'Memory req',
              accessor: (p) => p.memory_requests,
              render: (p) => p.memory_requests || '-',
              align: 'right',
              sortable: true,
            },
            {
              key: 'gpu',
              header: 'GPU',
              accessor: (p) => p.gpu_requests,
              render: (p) => (
                <span style={{ fontWeight: p.gpu_requests > 0 ? 600 : 400 }}>
                  {p.gpu_requests}
                </span>
              ),
              align: 'right',
              width: '60px',
              sortable: true,
            },
            {
              key: 'containers',
              header: t('mec.tenant.resources.colContainers'),
              accessor: (p) => p.ready_containers,
              render: (p) => `${p.ready_containers}/${p.containers}`,
              align: 'right',
              width: '90px',
              sortable: true,
            },
          ]}
        />
      </Section>

      <Section title="Services" marginTop="20px">
        <SortableTable<ServiceRow>
          data={data?.services || []}
          defaultSortKey="name"
          rowKey={(s) => s.name}
          emptyMessage={t('mec.tenant.resources.servicesEmpty')}
          columns={[
            {
              key: 'name',
              header: t('mec.tenant.resources.colName'),
              accessor: (s) => s.name,
              render: (s) => <code>{s.name}</code>,
              sortable: true,
            },
            {
              key: 'ip',
              header: 'External IP',
              accessor: (s) => s.external_ip,
              render: (s) => <code>{s.external_ip}</code>,
              sortable: true,
            },
            {
              key: 'ports',
              header: 'Ports',
              accessor: (s) => s.ports.map((p) => p.port).join(','),
              render: (s) =>
                s.ports.map((p) => `${p.port}/${p.protocol}`).join(', '),
              sortable: true,
            },
            {
              key: 'selector',
              header: 'Selector',
              accessor: (s) => s.selector_summary,
              render: (s) => (
                <span style={{ fontSize: '11px', fontFamily: 'monospace' }}>
                  {s.selector_summary}
                </span>
              ),
              sortable: true,
            },
            {
              key: 'age',
              header: 'Age',
              accessor: (s) => s.age_seconds,
              render: (s) => humanize(s.age_seconds),
              width: '80px',
              align: 'right',
              sortable: true,
            },
          ]}
        />
      </Section>
    </div>
  );
}

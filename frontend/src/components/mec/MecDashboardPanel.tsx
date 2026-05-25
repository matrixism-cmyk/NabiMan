import React from 'react';
import { useMecApi, useMecList } from '../../hooks/mec/useMecApi';
import { AuditLog, DashboardSummary } from '../../types/mec';
import {
  BarChart,
  DonutChart,
  ErrorBanner,
  MetricCard,
  MetricGrid,
  PanelLayout,
  Section,
  SortableTable,
  StatusBadge,
} from './common';

const REFRESH_INTERVAL = 30_000;

function pct(x: number, total: number): number {
  return total > 0 ? Math.round((x / total) * 100) : 0;
}

export default function MecDashboardPanel() {
  const summaryQuery = useMecApi<DashboardSummary>(
    '/api/mec/v1/dashboard/summary',
    REFRESH_INTERVAL,
  );
  const activityQuery = useMecList<AuditLog>(
    '/api/mec/v1/dashboard/activity',
    REFRESH_INTERVAL,
  );

  const s = summaryQuery.data;
  const refresh = () => {
    summaryQuery.refetch();
    activityQuery.refetch();
  };

  return (
    <PanelLayout
      title="MEC 통합 대시보드"
      subtitle="실시간 클러스터 자원 현황과 최근 운영 활동 요약"
      actions={
        <button className="btn btn-secondary" onClick={refresh}>
          새로고침
        </button>
      }
    >
      <ErrorBanner error={summaryQuery.error || undefined} />

      {!s && summaryQuery.loading && (
        <div style={{ color: '#6b7280' }}>로딩 중...</div>
      )}

      {s && (
        <>
          <MetricGrid>
            <MetricCard
              label="테넌트"
              value={s.tenants.active}
              unit={`/ ${s.tenants.total}`}
              helper={`활성 ${s.tenants.active}곳 · 전체 ${s.tenants.total}곳`}
              tone={s.tenants.active === s.tenants.total ? 'success' : 'info'}
            />
            <MetricCard
              label="노드 Ready"
              value={s.nodes.ready}
              unit={`/ ${s.nodes.total}`}
              helper={`${pct(s.nodes.ready, s.nodes.total)}% 정상`}
              tone={s.nodes.ready === s.nodes.total ? 'success' : 'warning'}
            />
            <MetricCard
              label="GPU Slot"
              value={s.gpu.allocated_slots}
              unit={`/ ${s.gpu.total_slots}`}
              helper={`여유 ${s.gpu.available_slots} slots`}
              tone={s.gpu.available_slots === 0 ? 'error' : 'info'}
            />
            <MetricCard
              label="LB Services"
              value={s.network.lb_services}
              helper={`공인 IP 할당 ${s.network.public_ips_assigned}개`}
              tone="info"
            />
            <MetricCard
              label="NAT 규칙"
              value={s.firewall.nat_rules}
              helper={`보안 정책 ${s.firewall.security_policies}개`}
              tone="info"
            />
          </MetricGrid>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
              gap: '16px',
              marginTop: '20px',
            }}
          >
            <Section title="GPU Slot 할당">
              <DonutChart
                slices={[
                  { label: '할당', value: s.gpu.allocated_slots, color: '#3b82f6' },
                  { label: '여유', value: s.gpu.available_slots, color: '#e5e7eb' },
                ]}
                centerLabel={`${pct(s.gpu.allocated_slots, s.gpu.total_slots)}%`}
                centerSublabel={`${s.gpu.allocated_slots}/${s.gpu.total_slots}`}
                legend="side"
              />
            </Section>

            <Section title="노드 상태">
              <DonutChart
                slices={[
                  { label: 'Ready', value: s.nodes.ready, color: '#10b981' },
                  {
                    label: 'Not Ready',
                    value: Math.max(0, s.nodes.total - s.nodes.ready),
                    color: '#ef4444',
                  },
                ]}
                centerLabel={`${s.nodes.ready}`}
                centerSublabel={`총 ${s.nodes.total} 노드`}
                legend="side"
              />
            </Section>

            <Section title="자원 사용률">
              <BarChart
                data={[
                  {
                    label: 'GPU Slots',
                    value: s.gpu.allocated_slots,
                    max: s.gpu.total_slots,
                  },
                  {
                    label: '활성 테넌트',
                    value: s.tenants.active,
                    max: Math.max(1, s.tenants.total),
                  },
                  {
                    label: '정상 노드',
                    value: s.nodes.ready,
                    max: Math.max(1, s.nodes.total),
                  },
                ]}
              />
            </Section>
          </div>

          <Section title="최근 활동" marginTop="24px">
            {activityQuery.error && (
              <div style={{ color: '#ef4444', fontSize: '13px' }}>
                {activityQuery.error}
              </div>
            )}
            <SortableTable<AuditLog>
              data={activityQuery.data || []}
              defaultSortKey="timestamp"
              defaultSortDir="desc"
              rowKey={(r) => r.id}
              emptyMessage="기록된 활동이 없습니다."
              columns={[
                {
                  key: 'timestamp',
                  header: '시간',
                  accessor: (r) => r.timestamp,
                  render: (r) => new Date(r.timestamp).toLocaleString(),
                  width: '170px',
                  sortable: true,
                },
                {
                  key: 'user',
                  header: '사용자',
                  accessor: (r) => r.user,
                  width: '100px',
                  sortable: true,
                },
                {
                  key: 'action',
                  header: '작업',
                  accessor: (r) => r.action,
                  render: (r) => <code style={{ fontSize: '12px' }}>{r.action}</code>,
                  sortable: true,
                },
                {
                  key: 'resource',
                  header: '대상',
                  accessor: (r) => `${r.resource_type}:${r.resource_id}`,
                  render: (r) => (
                    <>
                      <span style={{ color: '#6b7280' }}>{r.resource_type}</span>
                      <span style={{ color: '#d1d5db', margin: '0 4px' }}>/</span>
                      <code>{r.resource_id}</code>
                    </>
                  ),
                  sortable: true,
                },
                {
                  key: 'duration_ms',
                  header: '소요',
                  accessor: (r) => r.duration_ms,
                  render: (r) => `${r.duration_ms}ms`,
                  align: 'right',
                  width: '80px',
                  sortable: true,
                },
                {
                  key: 'status',
                  header: '결과',
                  accessor: (r) => r.status,
                  render: (r) => <StatusBadge tone={r.status}>{r.status}</StatusBadge>,
                  width: '90px',
                  sortable: true,
                },
              ]}
            />
          </Section>
        </>
      )}
    </PanelLayout>
  );
}

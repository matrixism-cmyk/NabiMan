import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { useMecApi, useMecList } from '../../hooks/mec/useMecApi';
import { useMetricHistory, trendOf } from '../../hooks/mec/useMetricHistory';
import { AuditLog, DashboardSummary } from '../../types/mec';
import {
  BarChart,
  DonutChart,
  ErrorBanner,
  LiveIndicator,
  MetricCard,
  MetricGrid,
  PanelLayout,
  Section,
  SortableTable,
  StatusBadge,
} from './common';

const REFRESH_INTERVAL = 15_000;

interface Props {
  /** Navigate to another MEC tab (drill-down from a metric card). */
  onNavigate?: (tab: string) => void;
}

function pct(x: number, total: number): number {
  return total > 0 ? Math.round((x / total) * 100) : 0;
}

export default function MecDashboardPanel({ onNavigate }: Props) {
  const { t } = useT();
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
  const go = (tab: string) => onNavigate?.(tab);

  // Timestamp of the latest successful summary load (drives the LIVE label).
  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  useEffect(() => {
    if (s) setLastUpdated(Date.now());
  }, [s]);

  // Rolling history for the per-card sparklines + trend arrows.
  const metrics = useMemo(
    () =>
      s
        ? {
            tenants: s.tenants.active,
            nodes: s.nodes.ready,
            gpu: s.gpu.allocated_slots,
            lb: s.network.lb_services,
            nat: s.firewall.nat_rules,
          }
        : null,
    [s],
  );
  const history = useMetricHistory('mec-dashboard-history', metrics);

  // Animate genuinely new activity rows (skip the very first load).
  const activity = useMemo(() => activityQuery.data || [], [activityQuery.data]);
  const seen = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());
  useEffect(() => {
    const incoming = new Set<string>();
    for (const a of activity) if (!seen.current.has(a.id)) incoming.add(a.id);
    if (incoming.size && seen.current.size) {
      setNewKeys(incoming);
      const t = setTimeout(() => setNewKeys(new Set()), 1000);
      activity.forEach((a) => seen.current.add(a.id));
      return () => clearTimeout(t);
    }
    activity.forEach((a) => seen.current.add(a.id));
  }, [activity]);

  return (
    <PanelLayout
      title={t('mec.dash.title')}
      subtitle={t('mec.dash.subtitle')}
      actions={
        <>
          <LiveIndicator
            lastUpdated={lastUpdated}
            intervalMs={REFRESH_INTERVAL}
            refreshing={summaryQuery.loading}
          />
          <button className="btn btn-secondary" onClick={refresh}>
            {t('mec.action.refresh')}
          </button>
        </>
      }
    >
      <ErrorBanner error={summaryQuery.error || undefined} />

      {!s && summaryQuery.loading && (
        <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>
      )}

      {s && (
        <div className="mec-fade-in-up">
          <MetricGrid>
            <MetricCard
              label={t('mec.dash.tenants')}
              value={s.tenants.active}
              unit={`/ ${s.tenants.total}`}
              helper={t('mec.dash.tenantsHelper', { active: s.tenants.active, total: s.tenants.total })}
              tone={s.tenants.active === s.tenants.total ? 'success' : 'info'}
              trend={trendOf(history.tenants)}
              sparkline={history.tenants}
              onClick={() => go('mecTenants')}
            />
            <MetricCard
              label={t('mec.dash.nodesReady')}
              value={s.nodes.ready}
              unit={`/ ${s.nodes.total}`}
              helper={t('mec.dash.nodesReadyHelper', { pct: pct(s.nodes.ready, s.nodes.total) })}
              tone={s.nodes.ready === s.nodes.total ? 'success' : 'warning'}
              trend={trendOf(history.nodes)}
              sparkline={history.nodes}
              onClick={() => go('mecNodes')}
            />
            <MetricCard
              label={t('mec.dash.gpuSlot')}
              value={s.gpu.allocated_slots}
              unit={`/ ${s.gpu.total_slots}`}
              helper={t('mec.dash.gpuSlotHelper', { available: s.gpu.available_slots })}
              tone={s.gpu.available_slots === 0 ? 'error' : 'info'}
              trend={trendOf(history.gpu)}
              sparkline={history.gpu}
              onClick={() => go('mecGpu')}
            />
            <MetricCard
              label={t('mec.dash.lbServices')}
              value={s.network.lb_services}
              helper={t('mec.dash.lbServicesHelper', { count: s.network.public_ips_assigned })}
              tone="info"
              trend={trendOf(history.lb)}
              sparkline={history.lb}
              onClick={() => go('mecIngress')}
            />
            <MetricCard
              label={t('mec.dash.natRules')}
              value={s.firewall.nat_rules}
              helper={t('mec.dash.natRulesHelper', { count: s.firewall.security_policies })}
              tone="info"
              trend={trendOf(history.nat)}
              sparkline={history.nat}
              onClick={() => go('mecFirewall')}
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
            <Section title={t('mec.dash.gpuAllocation')}>
              <DonutChart
                slices={[
                  { label: t('mec.dash.gpuAllocated'), value: s.gpu.allocated_slots, color: '#3b82f6' },
                  { label: t('mec.dash.gpuAvailable'), value: s.gpu.available_slots, color: '#e5e7eb' },
                ]}
                centerLabel={`${pct(s.gpu.allocated_slots, s.gpu.total_slots)}%`}
                centerSublabel={`${s.gpu.allocated_slots}/${s.gpu.total_slots}`}
                legend="side"
              />
            </Section>

            <Section title={t('mec.dash.nodeStatus')}>
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
                centerSublabel={t('mec.dash.nodeTotal', { total: s.nodes.total })}
                legend="side"
              />
            </Section>

            <Section title={t('mec.dash.resourceUsage')}>
              <BarChart
                data={[
                  {
                    label: 'GPU Slots',
                    value: s.gpu.allocated_slots,
                    max: s.gpu.total_slots,
                  },
                  {
                    label: t('mec.dash.activeTenants'),
                    value: s.tenants.active,
                    max: Math.max(1, s.tenants.total),
                  },
                  {
                    label: t('mec.dash.healthyNodes'),
                    value: s.nodes.ready,
                    max: Math.max(1, s.nodes.total),
                  },
                ]}
              />
            </Section>
          </div>

          <Section title={t('mec.dash.recentActivity')} marginTop="24px">
            {activityQuery.error && (
              <div style={{ color: '#ef4444', fontSize: '13px' }}>
                {activityQuery.error}
              </div>
            )}
            <SortableTable<AuditLog>
              data={activity}
              defaultSortKey="timestamp"
              defaultSortDir="desc"
              rowKey={(r) => r.id}
              highlightRowKeys={newKeys}
              emptyMessage={t('mec.dash.noActivity')}
              columns={[
                {
                  key: 'timestamp',
                  header: t('mec.col.time'),
                  accessor: (r) => r.timestamp,
                  render: (r) => new Date(r.timestamp).toLocaleString(),
                  width: '170px',
                  sortable: true,
                },
                {
                  key: 'user',
                  header: t('mec.dash.colUser'),
                  accessor: (r) => r.user,
                  width: '100px',
                  sortable: true,
                },
                {
                  key: 'action',
                  header: t('mec.dash.colAction'),
                  accessor: (r) => r.action,
                  render: (r) => <code style={{ fontSize: '12px' }}>{r.action}</code>,
                  sortable: true,
                },
                {
                  key: 'resource',
                  header: t('mec.col.object'),
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
                  header: t('mec.dash.colDuration'),
                  accessor: (r) => r.duration_ms,
                  render: (r) => `${r.duration_ms}ms`,
                  align: 'right',
                  width: '80px',
                  sortable: true,
                },
                {
                  key: 'status',
                  header: t('mec.dash.colResult'),
                  accessor: (r) => r.status,
                  render: (r) => <StatusBadge tone={r.status}>{r.status}</StatusBadge>,
                  width: '90px',
                  sortable: true,
                },
              ]}
            />
          </Section>
        </div>
      )}
    </PanelLayout>
  );
}

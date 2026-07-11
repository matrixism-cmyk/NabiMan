import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { useDashboardStream } from '../../hooks/mec/useDashboardStream';
import { useMetricHistory } from '../../hooks/mec/useMetricHistory';
import { ClusterEvent, LiveSnapshot } from '../../types/mec';
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
} from './common';

const REFRESH_INTERVAL = 10_000;

const cores = (milli: number) => (milli / 1000).toFixed(milli >= 10_000 ? 0 : 1);
const gib = (bytes: number) => (bytes / 1_073_741_824).toFixed(1);
const tone = (pct: number) => (pct >= 90 ? 'error' : pct >= 70 ? 'warning' : 'success');
const eventKey = (e: ClusterEvent) =>
  `${e.reason}|${e.kind}|${e.namespace || ''}/${e.name}|${e.last_time || ''}`;

export default function MecLivePanel() {
  const { t } = useT();
  // Shared server-pushed stream; falls back to direct polling if it degrades.
  const stream = useDashboardStream<LiveSnapshot>('/api/mec/v1/dashboard/live/stream');
  const poll = useMecApi<LiveSnapshot>(stream.degraded ? '/api/mec/v1/dashboard/live' : '', REFRESH_INTERVAL);
  const s = stream.data ?? poll.data;
  const err = stream.degraded ? (poll.error || stream.error) : null;

  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  useEffect(() => { if (s) setLastUpdated(Date.now()); }, [s]);

  const metrics = useMemo(
    () => (s ? { cpu: Math.round(s.cluster.cpu_percent), mem: Math.round(s.cluster.memory_percent) } : null),
    [s],
  );
  const history = useMetricHistory('mec-live-history', metrics);

  // Slide-in newly arrived events.
  const events = useMemo(() => s?.events || [], [s]);
  const seen = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());
  useEffect(() => {
    const incoming = new Set<string>();
    for (const e of events) { const k = eventKey(e); if (!seen.current.has(k)) incoming.add(k); }
    if (incoming.size && seen.current.size) {
      setNewKeys(incoming);
      const t = setTimeout(() => setNewKeys(new Set()), 1000);
      events.forEach((e) => seen.current.add(eventKey(e)));
      return () => clearTimeout(t);
    }
    events.forEach((e) => seen.current.add(eventKey(e)));
  }, [events]);

  return (
    <PanelLayout
      title={t('mec.live.title')}
      subtitle={t('mec.live.subtitle')}
      actions={
        <>
          <LiveIndicator lastUpdated={lastUpdated} intervalMs={REFRESH_INTERVAL} refreshing={stream.degraded ? poll.loading : !stream.connected} />
          <button className="btn btn-secondary" onClick={() => poll.refetch()}>{t('mec.action.refresh')}</button>
        </>
      }
    >
      <ErrorBanner error={err || undefined} />
      {!s && !err && <div style={{ color: '#6b7280' }}>{t('mec.state.loading')}</div>}

      {s && (
        <div className="mec-fade-in-up">
          <MetricGrid>
            <MetricCard
              label={t('mec.live.cpuCard')}
              value={Math.round(s.cluster.cpu_percent)}
              unit="%"
              helper={`${cores(s.cluster.cpu_used_millicores)} / ${cores(s.cluster.cpu_capacity_millicores)} cores`}
              tone={tone(s.cluster.cpu_percent)}
              sparkline={history.cpu}
            />
            <MetricCard
              label={t('mec.live.memCard')}
              value={Math.round(s.cluster.memory_percent)}
              unit="%"
              helper={`${gib(s.cluster.memory_used_bytes)} / ${gib(s.cluster.memory_capacity_bytes)} GiB`}
              tone={tone(s.cluster.memory_percent)}
              sparkline={history.mem}
            />
            <MetricCard
              label={t('mec.live.podsRunning')}
              value={s.pods.running}
              unit={`/ ${s.pods.total}`}
              helper={t('mec.live.podsHelper', { pending: s.pods.pending, failed: s.pods.failed, succeeded: s.pods.succeeded })}
              tone={s.pods.failed > 0 ? 'warning' : 'success'}
            />
            <MetricCard
              label={t('mec.live.nodes')}
              value={s.cluster.node_count}
              helper={t('mec.live.nodesHelper')}
              tone="info"
            />
          </MetricGrid>

          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))',
              gap: '16px',
              marginTop: '20px',
            }}
          >
            <Section title={t('mec.live.nodeCpu')}>
              <BarChart
                data={s.nodes.map((n) => ({
                  label: n.name,
                  value: Math.round(n.cpu_usage_percent),
                  max: 100,
                  helper: `${cores(n.cpu_usage_millicores)} / ${cores(n.cpu_capacity_millicores)} cores`,
                }))}
              />
            </Section>
            <Section title={t('mec.live.nodeMem')}>
              <BarChart
                data={s.nodes.map((n) => ({
                  label: n.name,
                  value: Math.round(n.memory_usage_percent),
                  max: 100,
                  helper: `${gib(n.memory_usage_bytes)} / ${gib(n.memory_capacity_bytes)} GiB`,
                }))}
              />
            </Section>
            <Section title={t('mec.live.podDist')}>
              <DonutChart
                slices={[
                  { label: 'Running', value: s.pods.running, color: '#10b981' },
                  { label: 'Pending', value: s.pods.pending, color: '#f59e0b' },
                  { label: 'Failed', value: s.pods.failed, color: '#ef4444' },
                  { label: 'Succeeded', value: s.pods.succeeded, color: '#3b82f6' },
                ]}
                centerLabel={`${s.pods.running}`}
                centerSublabel={t('mec.live.podTotal', { total: s.pods.total })}
                legend="side"
              />
            </Section>
          </div>

          <Section title={t('mec.live.nsConsumption')} marginTop="24px">
            <BarChart
              data={s.tenants.map((t) => ({
                label: t.namespace,
                value: Math.round(t.cpu_millicores),
                helper: `${gib(t.memory_bytes)} GiB · ${t.pods} pods`,
              }))}
              showValue
            />
          </Section>

          <Section title={t('mec.live.events')} marginTop="24px">
            <SortableTable<ClusterEvent>
              data={events}
              rowKey={eventKey}
              highlightRowKeys={newKeys}
              defaultSortKey="time"
              defaultSortDir="desc"
              emptyMessage={t('mec.live.noEvents')}
              columns={[
                {
                  key: 'time',
                  header: t('mec.col.time'),
                  accessor: (e) => e.last_time || '',
                  render: (e) => (e.last_time ? new Date(e.last_time).toLocaleTimeString() : '-'),
                  width: '90px',
                  sortable: true,
                },
                {
                  key: 'type',
                  header: t('mec.col.type'),
                  accessor: (e) => e.event_type,
                  render: (e) => (
                    <span
                      style={{
                        fontSize: '11px',
                        fontWeight: 600,
                        color: e.event_type === 'Warning' ? '#ef4444' : '#10b981',
                      }}
                    >
                      {e.event_type === 'Warning' ? '⚠ Warning' : 'Normal'}
                    </span>
                  ),
                  width: '90px',
                  sortable: true,
                },
                {
                  key: 'reason',
                  header: t('mec.col.reason'),
                  accessor: (e) => e.reason,
                  render: (e) => <code style={{ fontSize: '12px' }}>{e.reason}</code>,
                  width: '120px',
                  sortable: true,
                },
                {
                  key: 'object',
                  header: t('mec.col.object'),
                  accessor: (e) => `${e.kind}/${e.name}`,
                  render: (e) => (
                    <>
                      <span style={{ color: '#6b7280' }}>{e.namespace ? `${e.namespace}/` : ''}</span>
                      <span style={{ color: '#9ca3af' }}>{e.kind} </span>
                      <code>{e.name}</code>
                    </>
                  ),
                  sortable: true,
                },
                {
                  key: 'message',
                  header: t('mec.col.message'),
                  accessor: (e) => e.message,
                  wrap: true,
                  sortable: false,
                },
              ]}
            />
          </Section>
        </div>
      )}
    </PanelLayout>
  );
}

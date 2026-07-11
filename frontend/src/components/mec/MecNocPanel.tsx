import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { useMecApi } from '../../hooks/mec/useMecApi';
import { useDashboardStream } from '../../hooks/mec/useDashboardStream';
import { useMetricHistory } from '../../hooks/mec/useMetricHistory';
import { useNocSignals } from '../../hooks/mec/useNocSignals';
import { useStaleWatchdog } from '../../hooks/mec/useStaleWatchdog';
import { useWakeLock } from '../../hooks/mec/useWakeLock';
import { ClusterEvent, LiveSnapshot, VpnSession } from '../../types/mec';
import {
  BarChart, DonutChart, ErrorBanner, LiveIndicator, MetricCard, MetricGrid,
  PanelLayout, Section, SortableTable, StatusBadge,
} from './common';
import SeverityRollup from './noc/SeverityRollup';
import NocNodeWall from './noc/NocNodeWall';
import NocAttentionQueue from './noc/NocAttentionQueue';
import NocAlertMarquee from './noc/NocAlertMarquee';

const REFRESH = 10_000;
const STALE_MS = 40_000;
const OWNER = 'the@aeokorea.com';

interface Props {
  onNavigate?: (tab: string) => void;
  onEnterFocus?: () => void;
}

const cores = (m: number) => (m / 1000).toFixed(m >= 10_000 ? 0 : 1);
const gib = (b: number) => (b / 1_073_741_824).toFixed(1);
const t70 = (p: number) => (p >= 90 ? 'error' : p >= 70 ? 'warning' : 'success');
const eventKey = (e: ClusterEvent) => `${e.reason}|${e.kind}|${e.namespace || ''}/${e.name}|${e.last_time || ''}`;

function PlaceholderBand({ title, hint }: { title: string; hint: string }) {
  const { t } = useT();
  return (
    <div style={{ border: '1px dashed var(--border)', borderRadius: '8px', padding: '16px',
      color: 'var(--text-secondary)', background: 'var(--surface)' }}>
      <div style={{ fontSize: '13px', fontWeight: 600, color: 'var(--text)', marginBottom: '4px' }}>{title}</div>
      <div style={{ fontSize: '12px' }}>{t('noc.notConnected')} · {hint}</div>
    </div>
  );
}

export default function MecNocPanel({ onNavigate, onEnterFocus }: Props) {
  const { t } = useT();
  // Primary: one shared server-pushed stream (all walls share a single kube poll
  // set). Fallback: if the stream can't hold a connection, poll /live directly.
  const stream = useDashboardStream<LiveSnapshot>('/api/mec/v1/dashboard/live/stream');
  const poll = useMecApi<LiveSnapshot>(stream.degraded ? '/api/mec/v1/dashboard/live' : '', REFRESH);
  const s = stream.data ?? poll.data;
  const err = stream.degraded ? (poll.error || stream.error) : null;

  const [lastUpdated, setLastUpdated] = useState<number | null>(null);
  useEffect(() => { if (s) setLastUpdated(Date.now()); }, [s]);
  const stale = useStaleWatchdog(lastUpdated, STALE_MS);
  useWakeLock(true);

  const metrics = useMemo(
    () => (s ? { cpu: Math.round(s.cluster.cpu_percent), mem: Math.round(s.cluster.memory_percent) } : null),
    [s],
  );
  // In-memory only: an always-on wall shouldn't write localStorage every tick.
  const history = useMetricHistory('mec-noc-history', metrics, false);
  const { rollup, items } = useNocSignals(s ?? null, t);

  const events = useMemo(() => s?.events || [], [s]);
  // WS-E data sources light up automatically when connected; otherwise the
  // honest placeholders stay. GPU% needs dcgm-exporter; VPN needs AXGATE.
  const gpuNodes = useMemo(() => (s?.nodes || []).filter((n) => n.gpu_usage_percent != null), [s]);
  const vpn = useMemo<VpnSession[]>(() => s?.vpn_sessions || [], [s]);
  const seen = useRef<Set<string>>(new Set());
  const [newKeys, setNewKeys] = useState<Set<string>>(new Set());
  useEffect(() => {
    const incoming = new Set<string>();
    for (const e of events) { const k = eventKey(e); if (!seen.current.has(k)) incoming.add(k); }
    if (incoming.size && seen.current.size) {
      setNewKeys(incoming);
      const id = setTimeout(() => setNewKeys(new Set()), 1000);
      events.forEach((e) => seen.current.add(eventKey(e)));
      return () => clearTimeout(id);
    }
    events.forEach((e) => seen.current.add(eventKey(e)));
  }, [events]);

  const go = (tab: string) => onNavigate?.(tab);

  return (
    <PanelLayout
      title={t('noc.title')}
      subtitle={t('noc.subtitle')}
      actions={
        <>
          <SeverityRollup normal={rollup.normal} caution={rollup.caution} critical={rollup.critical} />
          <LiveIndicator lastUpdated={lastUpdated} intervalMs={REFRESH} refreshing={stream.degraded ? poll.loading : !stream.connected} />
          {onEnterFocus && <button className="btn btn-secondary" title={t('noc.fullscreen')} onClick={onEnterFocus}>⛶</button>}
          <button className="btn btn-secondary" onClick={() => poll.refetch()}>{t('noc.refresh')}</button>
        </>
      }
    >
      <ErrorBanner error={err || undefined} />
      {!s && !err && <div style={{ color: 'var(--text-secondary)' }}>{t('noc.loading')}</div>}

      {s && (
        <div className="mec-fade-in-up" style={stale ? { opacity: 0.5, filter: 'grayscale(0.6)', transition: 'opacity 0.3s' } : { transition: 'opacity 0.3s' }}>
          <MetricGrid>
            <MetricCard label={t('noc.kpi.cpu')} value={Math.round(s.cluster.cpu_percent)} unit="%"
              helper={t('noc.kpi.coresOf', { used: cores(s.cluster.cpu_used_millicores), total: cores(s.cluster.cpu_capacity_millicores) })}
              tone={t70(s.cluster.cpu_percent)} sparkline={history.cpu} />
            <MetricCard label={t('noc.kpi.memory')} value={Math.round(s.cluster.memory_percent)} unit="%"
              helper={t('noc.kpi.gibOf', { used: gib(s.cluster.memory_used_bytes), total: gib(s.cluster.memory_capacity_bytes) })}
              tone={t70(s.cluster.memory_percent)} sparkline={history.mem} />
            <MetricCard label={t('noc.kpi.pods')} value={s.pods.running} unit={`/ ${s.pods.total}`}
              helper={t('noc.kpi.podsHelper', { pending: s.pods.pending, failed: s.pods.failed })}
              tone={s.pods.failed > 0 ? 'warning' : 'success'} />
            <MetricCard label={t('noc.kpi.nodes')} value={s.health.nodes_ready} unit={`/ ${s.health.nodes_total}`}
              tone={s.health.nodes_ready === s.health.nodes_total ? 'success' : 'error'} />
            <MetricCard label={t('noc.kpi.gpu')} value={s.health.gpu_allocated_slots} unit={`/ ${s.health.gpu_total_slots}`}
              helper={t('noc.kpi.gpuHelper', { idle: s.health.gpu_available_slots })} tone="info" />
            <MetricCard label={t('noc.kpi.sessions')} value={vpn.length > 0 ? vpn.length : '—'}
              helper={vpn.length > 0 ? t('noc.vpnActive') : t('noc.notConnected')}
              tone={vpn.length > 0 ? 'info' : 'neutral'} />
          </MetricGrid>

          <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 2fr) minmax(260px, 1fr)', gap: '16px', marginTop: '20px' }}>
            <Section title={t('noc.nodeWall')}><NocNodeWall nodes={s.nodes} onPick={() => go('mecNodes')} /></Section>
            <Section title={t('noc.attention')}><NocAttentionQueue items={items} onPick={go} /></Section>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '16px', marginTop: '20px' }}>
            <Section title={t('noc.podPhases')}>
              <DonutChart legend="side"
                centerLabel={`${s.pods.running}`} centerSublabel={`/ ${s.pods.total}`}
                slices={[
                  { label: 'Running', value: s.pods.running, color: '#10b981' },
                  { label: 'Pending', value: s.pods.pending, color: '#f59e0b' },
                  { label: 'Failed', value: s.pods.failed, color: '#ef4444' },
                  { label: 'Succeeded', value: s.pods.succeeded, color: '#3b82f6' },
                ]} />
            </Section>
            <Section title={t('noc.tenants')}>
              <BarChart showValue data={s.tenants.map((tn) => ({
                label: tn.namespace, value: Math.round(tn.cpu_millicores),
                helper: t('noc.tenantHelper', { mem: gib(tn.memory_bytes), pods: tn.pods }),
              }))} />
            </Section>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))', gap: '16px', marginTop: '16px' }}>
            {gpuNodes.length > 0 ? (
              <Section title={t('noc.gpuBand')}>
                <BarChart showValue data={gpuNodes.map((n) => ({
                  label: n.name, value: Math.round(n.gpu_usage_percent as number), max: 100,
                }))} />
              </Section>
            ) : (
              <PlaceholderBand title={t('noc.gpuBand')} hint={t('noc.gpuHint')} />
            )}
            {vpn.length > 0 ? (
              <Section title={t('noc.sessionBand')}>
                <SortableTable<VpnSession>
                  data={vpn} rowKey={(v) => `${v.user}|${v.ip}`} defaultSortKey="user" maxHeight="220px"
                  columns={[
                    { key: 'user', header: t('noc.vpnUser'), accessor: (v) => v.user, sortable: true },
                    { key: 'ip', header: t('noc.vpnIp'), accessor: (v) => v.ip,
                      render: (v) => <code>{v.ip}</code> },
                    { key: 'source', header: t('noc.vpnSource'), accessor: (v) => v.source_ip,
                      render: (v) => <code>{v.source_ip}</code> },
                  ]} />
              </Section>
            ) : (
              <PlaceholderBand title={t('noc.sessionBand')} hint={t('noc.sessionHint')} />
            )}
          </div>

          <Section title={t('noc.events')} marginTop="20px">
            <SortableTable<ClusterEvent>
              data={events} rowKey={eventKey} highlightRowKeys={newKeys} stickyHeader
              defaultSortKey="time" defaultSortDir="desc" maxHeight="320px"
              columns={[
                { key: 'time', header: '⏱', accessor: (e) => e.last_time || '',
                  render: (e) => (e.last_time ? new Date(e.last_time).toLocaleTimeString() : '-'), width: '90px', sortable: true },
                { key: 'type', header: 'Type', accessor: (e) => e.event_type, width: '90px', sortable: true,
                  render: (e) => <StatusBadge tone={e.event_type === 'Warning' ? 'warning' : 'success'}>{e.event_type}</StatusBadge> },
                { key: 'reason', header: 'Reason', accessor: (e) => e.reason, width: '120px', sortable: true,
                  render: (e) => <code style={{ fontSize: '12px' }}>{e.reason}</code> },
                { key: 'object', header: 'Object', accessor: (e) => `${e.kind}/${e.name}`, sortable: true,
                  render: (e) => <><span style={{ color: '#6b7280' }}>{e.namespace ? `${e.namespace}/` : ''}{e.kind} </span><code>{e.name}</code></> },
                { key: 'message', header: 'Message', accessor: (e) => e.message, wrap: true, sortable: false },
              ]} />
          </Section>

          <div style={{ marginTop: '16px' }}><NocAlertMarquee items={items} owner={OWNER} /></div>
        </div>
      )}
    </PanelLayout>
  );
}

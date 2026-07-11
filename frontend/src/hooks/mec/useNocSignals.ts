import { useMemo } from 'react';
import { LiveSnapshot, NodeMetrics } from '../../types/mec';
import { StatusTone } from '../../components/mec/common';

// All NOC thresholds live here so views carry no business logic.
const CPU_CRIT = 90;
const CPU_WARN = 70;
const MEM_CRIT = 90;
const MEM_WARN = 70;
const WARN_EVENTS_CAUTION = 5;
const KUBECONFIG_CRIT_DAYS = 3; // expiry this close = incident risk
const KUBECONFIG_WARN_DAYS = 14;

export type Severity = 'critical' | 'caution' | 'ok';

export interface NocSignal {
  id: string;
  severity: Exclude<Severity, 'ok'>;
  tone: StatusTone; // error | warning
  text: string;
  target?: string; // tab key for drill-down
}

export interface NocSignals {
  rollup: { normal: number; caution: number; critical: number };
  items: NocSignal[];
}

type T = (key: string, vars?: Record<string, string | number>) => string;

/** Worst severity for a single node from its readiness + CPU/MEM pressure. */
export function nodeSeverity(n: NodeMetrics): Severity {
  if (n.status && n.status !== 'Ready') return 'critical';
  if (n.cpu_usage_percent >= CPU_CRIT || n.memory_usage_percent >= MEM_CRIT) return 'critical';
  if (n.cpu_usage_percent >= CPU_WARN || n.memory_usage_percent >= MEM_WARN) return 'caution';
  return 'ok';
}

function nodeReason(n: NodeMetrics, t: T): string {
  if (n.status && n.status !== 'Ready') return t('noc.sig.nodeNotReady');
  if (n.cpu_usage_percent >= CPU_WARN && n.cpu_usage_percent >= n.memory_usage_percent) {
    return `${t('noc.sig.nodeCpu')} (${Math.round(n.cpu_usage_percent)}%)`;
  }
  return `${t('noc.sig.nodeMem')} (${Math.round(n.memory_usage_percent)}%)`;
}

/** Derive the severity rollup + sorted actionable worklist from a snapshot. */
export function useNocSignals(snap: LiveSnapshot | null, t: T): NocSignals {
  return useMemo(() => {
    if (!snap) return { rollup: { normal: 0, caution: 0, critical: 0 }, items: [] };
    const items: NocSignal[] = [];
    let normal = 0;

    for (const n of snap.nodes) {
      const sev = nodeSeverity(n);
      if (sev === 'ok') { normal++; continue; }
      items.push({
        id: `node:${n.name}`,
        severity: sev,
        tone: sev === 'critical' ? 'error' : 'warning',
        text: `${n.name} — ${nodeReason(n, t)}`,
        target: 'mecNodes',
      });
    }

    if (snap.pods.failed > 0) {
      items.push({ id: 'pods:failed', severity: 'critical', tone: 'error',
        text: t('noc.sig.podFailed', { n: snap.pods.failed }), target: 'mecLive' });
    }
    if (snap.pods.pending > 0) {
      items.push({ id: 'pods:pending', severity: 'caution', tone: 'warning',
        text: t('noc.sig.podPending', { n: snap.pods.pending }), target: 'mecLive' });
    }
    const warnEvents = snap.events.filter((e) => e.event_type === 'Warning').length;
    if (warnEvents >= WARN_EVENTS_CAUTION) {
      items.push({ id: 'events:warn', severity: 'caution', tone: 'warning',
        text: `${t('noc.sig.eventWarn')} (${warnEvents})`, target: 'mecLive' });
    }
    // Early warning before an expired kubeconfig token 401s every MEC call.
    const kd = snap.health.kubeconfig_days;
    if (kd != null && kd <= KUBECONFIG_WARN_DAYS) {
      const crit = kd <= KUBECONFIG_CRIT_DAYS;
      items.push({ id: 'kubeconfig:expiry', severity: crit ? 'critical' : 'caution',
        tone: crit ? 'error' : 'warning',
        text: t('noc.sig.kubeExpiry', { n: kd }), target: 'mecSettings' });
    }

    items.sort((a, b) => (a.severity === b.severity ? 0 : a.severity === 'critical' ? -1 : 1));
    // Rollup reflects ALL problems (node + pod + event), not just node health,
    // so 장애/주의 counts never disagree with the attention queue.
    const critical = items.filter((i) => i.severity === 'critical').length;
    const caution = items.filter((i) => i.severity === 'caution').length;
    return { rollup: { normal, caution, critical }, items };
  }, [snap, t]);
}

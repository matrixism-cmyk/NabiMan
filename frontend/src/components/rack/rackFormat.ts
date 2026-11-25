import type { DeviceStatus, GpuStatus, Level, NodeStatus, RackDevice, RackLayout, RackNode } from './rackTypes';

/** How a device is drawn: its type, with servers that carry GPUs set apart. */
export type Look = 'server' | 'gpu' | 'switch' | 'firewall' | '5g' | 'storage' | 'external' | 'other';
export const LOOKS: Look[] = ['server', 'gpu', 'switch', 'firewall', '5g', 'storage', 'external'];

export function lookOf(device: RackDevice, nodes: Record<string, RackNode>): Look {
  if (device.type !== 'server') return device.type;
  const own = device.node ? nodes[device.node] : undefined;
  return device.gpu || own?.gpu ? 'gpu' : 'server';
}

/** Each level's mark: a different shape for each, so colour is never the only sign. */
export const LEVEL_GLYPH: Record<Level, string> = { ok: '●', warn: '▲', down: '✖', noreply: '⊘', unknown: '?', none: '○' };
export const LEVELS: Level[] = ['ok', 'warn', 'down', 'noreply', 'unknown', 'none'];
/** No reply from a machine that is not ours: a quiet ring, not an alarm. */
export const CALM_GLYPH = '–';

/** No reply, from somebody else's device: shown without alarm. */
export function isCalm(status: DeviceStatus | undefined): boolean {
  return Boolean(status && status.level === 'noreply' && status.external);
}

/** Milliseconds with one decimal under 10: 0.6ms, 12ms; quicker than can be shown: <0.1ms. */
export function latency(ms: number | null | undefined): string {
  if (ms === null || ms === undefined) return '';
  if (ms < 0.05) return '<0.1ms';
  return `${ms < 10 ? (Math.round(ms * 10) / 10).toFixed(1) : Math.round(ms)}ms`;
}

type Translate = (key: string, params?: Record<string, string | number>) => string;

/** The few words on a device's face about whether it answers; '' for a node's device. */
export function reachText(t: Translate, status: DeviceStatus | undefined): string {
  if (!status || status.nodes.length > 0) return '';
  switch (status.level) {
    case 'ok': return [t('rack.reach.ok'), latency(status.latency_ms)].filter(Boolean).join(' · ');
    case 'noreply': return t('rack.reach.noreply');
    case 'unknown': return t('rack.reach.unknown');
    case 'none': return status.reason === 'no_address' ? t('rack.reach.noAddress') : status.reason ? t('rack.level.none') : '';
    default: return '';
  }
}

const WHYS = ['no_address', 'check_none', 'address_refused', 'over_limit', 'not_in_time', 'ping_missing', 'ping_denied', 'no_route', 'ping_failed'];

/** A sentence for why there is no answer to show. */
export function reachWhy(t: Translate, reason: string | null): string {
  if (!reason) return '';
  return t(`rack.why.${WHYS.includes(reason) ? reason : 'other'}`);
}

/** How it was checked, in words. */
export function checkedBy(t: Translate, by: string | null): string {
  if (!by) return '';
  if (by === 'prometheus-icmp' || by === 'prometheus-tcp' || by === 'ping') return t(`rack.by.${by}`);
  const port = /^tcp:(\d+)$/.exec(by)?.[1];
  return port ? t('rack.by.tcp', { port }) : by;
}

export function nodesByName(layout: RackLayout): Record<string, RackNode> {
  return Object.fromEntries(layout.nodes.map((n) => [n.name, n]));
}

/** The nodes a device shows: its own, else the ones it may be. */
export function deviceNodes(device: RackDevice): string[] {
  return device.node ? [device.node] : device.node_candidates;
}

export const pct = (v: number | null | undefined): string => (v === null || v === undefined ? '–' : `${Math.round(v)}%`);
export const celsius = (v: number | null | undefined): string => (v === null || v === undefined ? '–' : `${Math.round(v)}℃`);
export const watts = (v: number | null | undefined): string => (v === null || v === undefined ? '–' : `${Math.round(v)}W`);

/** MiB as GiB with one decimal under 10. */
export function gib(mib: number | null | undefined): string {
  if (mib === null || mib === undefined) return '–';
  const g = mib / 1024;
  return g < 10 ? g.toFixed(1) : String(Math.round(g));
}

/** Physical GPUs on a node (a GPU cut into MIG slices is one). */
export function gpuCount(node: NodeStatus | undefined): number {
  return node ? new Set(node.gpus.map((g) => g.index)).size : 0;
}

/** The busiest GPU's utilisation, or null when none reports it (MIG slices do not). */
export function gpuPeak(node: NodeStatus | undefined): number | null {
  const values = (node?.gpus ?? []).map((g) => g.util_pct).filter((v): v is number => v !== null);
  return values.length ? Math.max(...values) : null;
}

/** The hottest reading on a node: its board or any GPU. */
export function hottest(node: NodeStatus | undefined): number | null {
  if (!node) return null;
  const values = [node.temp_c, ...node.gpus.map((g) => g.temp_c)].filter((v): v is number => v !== null);
  return values.length ? Math.max(...values) : null;
}

export function gpuName(gpu: GpuStatus): string {
  return gpu.mig_id === null ? `GPU ${gpu.index}` : `GPU ${gpu.index} · MIG ${gpu.mig_id}`;
}

/** A bar's tone for a value against the level it warns at. */
export function barTone(value: number | null, warnAt: number | null): 'ok' | 'warn' | 'idle' {
  if (value === null) return 'idle';
  return warnAt !== null && value >= warnAt ? 'warn' : 'ok';
}

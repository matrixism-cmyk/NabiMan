import { renderHook } from '@testing-library/react';
import { nodeSeverity, useNocSignals } from './useNocSignals';
import { LiveSnapshot, NodeMetrics } from '../../types/mec';

const t = (key: string) => key; // identity stub — we assert on severity/counts, not copy

function node(over: Partial<NodeMetrics>): NodeMetrics {
  return {
    name: 'n', status: 'Ready',
    cpu_usage_millicores: 0, cpu_capacity_millicores: 1000,
    memory_usage_bytes: 0, memory_capacity_bytes: 1000,
    cpu_usage_percent: 0, memory_usage_percent: 0, gpu_usage_percent: null,
    ...over,
  };
}

function snap(over: Partial<LiveSnapshot>): LiveSnapshot {
  return {
    cluster: {
      cpu_used_millicores: 0, cpu_capacity_millicores: 1000, memory_used_bytes: 0,
      memory_capacity_bytes: 1000, cpu_percent: 0, memory_percent: 0, node_count: 0,
    },
    health: { nodes_ready: 0, nodes_total: 0, gpu_total_slots: 0, gpu_allocated_slots: 0, gpu_available_slots: 0 },
    nodes: [],
    pods: { running: 0, pending: 0, failed: 0, succeeded: 0, unknown: 0, total: 0 },
    tenants: [],
    events: [],
    ...over,
  };
}

describe('nodeSeverity', () => {
  it('is ok for a Ready node under threshold', () => {
    expect(nodeSeverity(node({ cpu_usage_percent: 40, memory_usage_percent: 40 }))).toBe('ok');
  });
  it('is critical for a NotReady node regardless of usage', () => {
    expect(nodeSeverity(node({ status: 'NotReady', cpu_usage_percent: 1 }))).toBe('critical');
  });
  it('is critical at >= 90% CPU or MEM', () => {
    expect(nodeSeverity(node({ cpu_usage_percent: 95 }))).toBe('critical');
    expect(nodeSeverity(node({ memory_usage_percent: 92 }))).toBe('critical');
  });
  it('is caution in the 70-89% band', () => {
    expect(nodeSeverity(node({ cpu_usage_percent: 75 }))).toBe('caution');
  });
});

describe('useNocSignals', () => {
  it('derives a rollup that agrees with the attention queue', () => {
    const s = snap({
      nodes: [
        node({ name: 'ok1', cpu_usage_percent: 10 }),
        node({ name: 'ok2', cpu_usage_percent: 20 }),
        node({ name: 'down', status: 'NotReady' }),
      ],
      pods: { running: 5, pending: 1, failed: 3, succeeded: 0, unknown: 0, total: 9 },
      events: Array.from({ length: 5 }, () => ({
        last_time: null, event_type: 'Warning', reason: 'BackOff', kind: 'Pod',
        name: 'p', namespace: 'ns', message: 'm', count: 1,
      })),
    });
    const { result } = renderHook(() => useNocSignals(s, t));
    const { rollup, items } = result.current;

    // critical: NotReady node + failed pods ; caution: pending pods + warning events
    expect(rollup.critical).toBe(2);
    expect(rollup.caution).toBe(2);
    expect(rollup.normal).toBe(2);
    // rollup counts must never disagree with the queue
    expect(items.filter((i) => i.severity === 'critical')).toHaveLength(rollup.critical);
    expect(items.filter((i) => i.severity === 'caution')).toHaveLength(rollup.caution);
    // critical items are sorted ahead of caution
    expect(items[0].severity).toBe('critical');
  });

  it('is all-clear for a healthy snapshot', () => {
    const s = snap({ nodes: [node({ name: 'ok', cpu_usage_percent: 5 })] });
    const { result } = renderHook(() => useNocSignals(s, t));
    expect(result.current.rollup).toEqual({ normal: 1, caution: 0, critical: 0 });
    expect(result.current.items).toHaveLength(0);
  });

  it('returns an empty result for a null snapshot', () => {
    const { result } = renderHook(() => useNocSignals(null, t));
    expect(result.current.items).toHaveLength(0);
    expect(result.current.rollup).toEqual({ normal: 0, caution: 0, critical: 0 });
  });
});

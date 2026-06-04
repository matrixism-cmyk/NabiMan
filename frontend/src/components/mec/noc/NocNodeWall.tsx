import React, { useMemo } from 'react';
import { NodeMetrics } from '../../../types/mec';
import { nodeSeverity, Severity } from '../../../hooks/mec/useNocSignals';

interface Props {
  nodes: NodeMetrics[];
  onPick?: (node: string) => void;
}

const SEV_COLOR: Record<Severity, string> = {
  critical: '#ef4444',
  caution: '#f59e0b',
  ok: '#10b981',
};
const SEV_RANK: Record<Severity, number> = { critical: 0, caution: 1, ok: 2 };

function MiniBar({ label, pct, color }: { label: string; pct: number; color: string }) {
  const v = Math.max(0, Math.min(100, Math.round(pct)));
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '10px' }}>
      <span style={{ width: '26px', color: 'var(--text-secondary)' }}>{label}</span>
      <span style={{ flex: 1, height: '6px', background: 'var(--surface-alt)', borderRadius: '3px', overflow: 'hidden' }}>
        <span style={{ display: 'block', height: '100%', width: `${v}%`, background: color, transition: 'width 0.4s ease-out' }} />
      </span>
      <span style={{ width: '30px', textAlign: 'right', color: 'var(--text)', fontVariantNumeric: 'tabular-nums' }}>{v}%</span>
    </div>
  );
}

function Tile({ node, onPick }: { node: NodeMetrics; onPick?: (n: string) => void }) {
  const sev = nodeSeverity(node);
  const down = !!node.status && node.status !== 'Ready';
  const color = SEV_COLOR[sev];
  return (
    <button
      onClick={onPick ? () => onPick(node.name) : undefined}
      className={down ? 'noc-tile-down' : undefined}
      style={{
        textAlign: 'left', cursor: onPick ? 'pointer' : 'default',
        background: 'var(--surface)', border: '1px solid var(--border)',
        borderLeft: `3px solid ${color}`, borderRadius: '8px', padding: '10px 12px',
        display: 'flex', flexDirection: 'column', gap: '6px', color: 'var(--text)',
        animation: sev !== 'ok' ? 'mec-pop 0.5s ease-out' : undefined,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '6px' }}>
        <span style={{ fontSize: '12px', fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {node.name}
        </span>
        <span style={{ width: '8px', height: '8px', borderRadius: '50%', background: color, flexShrink: 0 }} />
      </div>
      {down ? (
        <div style={{ fontSize: '11px', color: SEV_COLOR.critical, fontWeight: 600 }}>DOWN · {node.status}</div>
      ) : (
        <>
          <MiniBar label="CPU" pct={node.cpu_usage_percent} color={color} />
          <MiniBar label="MEM" pct={node.memory_usage_percent} color={color} />
        </>
      )}
    </button>
  );
}

/// Bento wall of node tiles, worst-first so the most-pressured node leads.
export default function NocNodeWall({ nodes, onPick }: Props) {
  const sorted = useMemo(
    () => [...nodes].sort((a, b) => {
      const d = SEV_RANK[nodeSeverity(a)] - SEV_RANK[nodeSeverity(b)];
      return d !== 0 ? d : b.cpu_usage_percent - a.cpu_usage_percent;
    }),
    [nodes],
  );
  if (!sorted.length) return <div style={{ color: 'var(--text-secondary)', fontSize: '13px' }}>—</div>;
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(170px, 1fr))', gap: '10px' }}>
      {sorted.map((n) => <Tile key={n.name} node={n} onPick={onPick} />)}
    </div>
  );
}

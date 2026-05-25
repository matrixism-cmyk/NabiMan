import React from 'react';

export interface DonutSlice {
  label: string;
  value: number;
  color?: string;
}

interface Props {
  slices: DonutSlice[];
  size?: number;
  thickness?: number;
  centerLabel?: React.ReactNode;
  centerSublabel?: React.ReactNode;
  legend?: 'side' | 'below' | 'none';
}

const DEFAULT_COLORS = [
  '#3b82f6',
  '#10b981',
  '#f59e0b',
  '#ef4444',
  '#8b5cf6',
  '#06b6d4',
  '#ec4899',
  '#84cc16',
];

export default function DonutChart({
  slices,
  size = 160,
  thickness = 24,
  centerLabel,
  centerSublabel,
  legend = 'side',
}: Props) {
  const total = slices.reduce((s, x) => s + Math.max(0, x.value), 0);
  const radius = size / 2 - thickness / 2;
  const cx = size / 2;
  const cy = size / 2;

  let cumulative = 0;
  const paths = slices.map((s, i) => {
    const value = Math.max(0, s.value);
    const color = s.color || DEFAULT_COLORS[i % DEFAULT_COLORS.length];
    if (total <= 0) {
      return { path: '', color, slice: s };
    }
    const frac = value / total;
    const startAngle = (cumulative / total) * 2 * Math.PI - Math.PI / 2;
    cumulative += value;
    const endAngle = (cumulative / total) * 2 * Math.PI - Math.PI / 2;
    // Full-circle-safe path generation
    if (frac >= 0.999999) {
      return {
        path: `M ${cx - radius} ${cy} a ${radius} ${radius} 0 1 0 ${radius * 2} 0 a ${radius} ${radius} 0 1 0 -${radius * 2} 0`,
        color,
        slice: s,
      };
    }
    const x1 = cx + radius * Math.cos(startAngle);
    const y1 = cy + radius * Math.sin(startAngle);
    const x2 = cx + radius * Math.cos(endAngle);
    const y2 = cy + radius * Math.sin(endAngle);
    const largeArc = frac > 0.5 ? 1 : 0;
    return {
      path: `M ${x1} ${y1} A ${radius} ${radius} 0 ${largeArc} 1 ${x2} ${y2}`,
      color,
      slice: s,
    };
  });

  const svg = (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
      {total === 0 && (
        <circle
          cx={cx}
          cy={cy}
          r={radius}
          fill="none"
          stroke="var(--border)"
          strokeWidth={thickness}
        />
      )}
      {paths.map((p, i) =>
        p.path ? (
          <path
            key={i}
            d={p.path}
            fill="none"
            stroke={p.color}
            strokeWidth={thickness}
            strokeLinecap="butt"
          />
        ) : null,
      )}
      {(centerLabel || centerSublabel) && (
        <foreignObject x={0} y={0} width={size} height={size}>
          <div
            style={{
              width: size,
              height: size,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              textAlign: 'center',
            }}
          >
            {centerLabel && (
              <div
                style={{
                  fontSize: '22px',
                  fontWeight: 700,
                  color: 'var(--text)',
                  lineHeight: 1,
                }}
              >
                {centerLabel}
              </div>
            )}
            {centerSublabel && (
              <div
                style={{
                  fontSize: '11px',
                  color: 'var(--text-secondary)',
                  marginTop: '4px',
                }}
              >
                {centerSublabel}
              </div>
            )}
          </div>
        </foreignObject>
      )}
    </svg>
  );

  if (legend === 'none') return svg;

  const legendEl = (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: '4px',
        fontSize: '12px',
      }}
    >
      {paths.map((p, i) => (
        <div key={i} style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <span
            style={{
              width: '10px',
              height: '10px',
              borderRadius: '2px',
              background: p.color,
              flexShrink: 0,
            }}
          />
          <span style={{ color: 'var(--text)' }}>{p.slice.label}</span>
          <span
            style={{
              color: 'var(--text-secondary)',
              marginLeft: 'auto',
              fontVariantNumeric: 'tabular-nums',
            }}
          >
            {p.slice.value.toLocaleString()}
          </span>
        </div>
      ))}
    </div>
  );

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: legend === 'side' ? 'row' : 'column',
        gap: '16px',
        alignItems: 'center',
      }}
    >
      {svg}
      {legendEl}
    </div>
  );
}

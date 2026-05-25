import React from 'react';

interface Props {
  data: number[];
  width?: number;
  height?: number;
  color?: string;
  /** Draw a soft filled area under the line. */
  fill?: boolean;
}

/// Minimal trend line — pure SVG, no dependency. Renders nothing until there
/// are at least two samples so an empty card stays clean.
export default function Sparkline({
  data,
  width = 96,
  height = 28,
  color = '#3b82f6',
  fill = true,
}: Props) {
  if (!data || data.length < 2) return null;

  const min = Math.min(...data);
  const max = Math.max(...data);
  const range = max - min || 1;
  const pad = 2;
  const innerH = height - pad * 2;

  const pt = (v: number, i: number): [number, number] => {
    const x = (i / (data.length - 1)) * width;
    const y = pad + innerH - ((v - min) / range) * innerH;
    return [x, y];
  };

  const pts = data.map(pt);
  const line = pts.map(([x, y]) => `${x.toFixed(1)},${y.toFixed(1)}`).join(' ');
  const area = `0,${height} ${line} ${width},${height}`;
  const gid = `spark-${color.replace('#', '')}`;

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} aria-hidden>
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.28} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      {fill && <polygon points={area} fill={`url(#${gid})`} stroke="none" />}
      <polyline
        points={line}
        fill="none"
        stroke={color}
        strokeWidth={1.5}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <circle cx={pts[pts.length - 1][0]} cy={pts[pts.length - 1][1]} r={2} fill={color} />
    </svg>
  );
}

import React from 'react';

export interface BarDatum {
  label: string;
  value: number;
  max?: number;
  color?: string;
  helper?: React.ReactNode;
}

interface Props {
  data: BarDatum[];
  maxOverall?: number;
  height?: number;
  showValue?: boolean;
  ariaLabel?: string;
}

/// Horizontal bar with label on the left, value on the right.
/// Pure SVG/divs, no chart library dependency.
function thresholdColor(ratio: number): string {
  if (ratio >= 0.9) return '#ef4444';
  if (ratio >= 0.7) return '#f59e0b';
  return '#10b981';
}

export default function BarChart({ data, maxOverall, showValue = true }: Props) {
  const globalMax = maxOverall ?? Math.max(1, ...data.map((d) => d.max ?? d.value));
  return (
    <div aria-label="bar chart" style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
      {data.map((d, i) => {
        const max = d.max ?? globalMax;
        const ratio = max > 0 ? Math.min(1, d.value / max) : 0;
        const color = d.color || thresholdColor(ratio);
        return (
          <div key={`${d.label}-${i}`}>
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                fontSize: '12px',
                marginBottom: '2px',
              }}
            >
              <span style={{ color: 'var(--text)', fontWeight: 500 }}>
                {d.label}
              </span>
              {showValue && (
                <span
                  style={{
                    color: 'var(--text-secondary)',
                    fontVariantNumeric: 'tabular-nums',
                  }}
                >
                  {d.value.toLocaleString()}
                  {d.max !== undefined && ` / ${d.max.toLocaleString()}`}
                  {' '}
                  <span style={{ color: 'var(--text-secondary)', opacity: 0.7 }}>
                    ({Math.round(ratio * 100)}%)
                  </span>
                </span>
              )}
            </div>
            <div
              style={{
                background: 'var(--surface-alt)',
                borderRadius: '3px',
                height: '10px',
                overflow: 'hidden',
                position: 'relative',
              }}
              role="progressbar"
              aria-valuenow={d.value}
              aria-valuemax={max}
              aria-valuemin={0}
            >
              <div
                style={{
                  background: color,
                  height: '100%',
                  width: `${ratio * 100}%`,
                  transition: 'width 0.4s ease-out',
                }}
              />
            </div>
            {d.helper && (
              <div
                style={{
                  fontSize: '11px',
                  color: 'var(--text-secondary)',
                  opacity: 0.8,
                  marginTop: '2px',
                }}
              >
                {d.helper}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

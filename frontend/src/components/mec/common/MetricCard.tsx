import React from 'react';
import { StatusTone, tone as toneFor } from './StatusBadge';

interface Props {
  label: string;
  value: React.ReactNode;
  unit?: string;
  helper?: React.ReactNode;
  trend?: 'up' | 'down' | 'flat';
  tone?: StatusTone | string;
  icon?: React.ReactNode;
  onClick?: () => void;
}

const TONE_BG: Record<StatusTone, string> = {
  success: '#10b981',
  warning: '#f59e0b',
  error: '#ef4444',
  info: '#3b82f6',
  neutral: '#6b7280',
  running: '#8b5cf6',
};

export default function MetricCard({
  label,
  value,
  unit,
  helper,
  trend,
  tone: toneProp,
  icon,
  onClick,
}: Props) {
  const t: StatusTone =
    typeof toneProp === 'string' && toneProp in TONE_BG
      ? (toneProp as StatusTone)
      : toneFor(toneProp as any);
  const accent = TONE_BG[t];

  return (
    <div
      onClick={onClick}
      style={{
        background: 'var(--surface)',
        borderRadius: '8px',
        border: '1px solid var(--border)',
        borderLeft: `3px solid ${accent}`,
        padding: '14px 16px',
        cursor: onClick ? 'pointer' : undefined,
        transition: 'transform 0.1s, box-shadow 0.1s',
        boxShadow: '0 1px 2px rgba(0,0,0,0.1)',
        color: 'var(--text)',
      }}
      onMouseEnter={
        onClick
          ? (e) => {
              (e.currentTarget as HTMLElement).style.transform = 'translateY(-1px)';
              (e.currentTarget as HTMLElement).style.boxShadow =
                '0 4px 8px rgba(0,0,0,0.2)';
            }
          : undefined
      }
      onMouseLeave={
        onClick
          ? (e) => {
              (e.currentTarget as HTMLElement).style.transform = '';
              (e.currentTarget as HTMLElement).style.boxShadow =
                '0 1px 2px rgba(0,0,0,0.1)';
            }
          : undefined
      }
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          marginBottom: '6px',
        }}
      >
        <span
          style={{
            fontSize: '11px',
            color: 'var(--text-secondary)',
            fontWeight: 500,
            textTransform: 'uppercase',
            letterSpacing: '0.05em',
          }}
        >
          {label}
        </span>
        {icon && (
          <span style={{ color: accent, display: 'inline-flex' }}>{icon}</span>
        )}
      </div>
      <div
        style={{
          fontSize: '24px',
          fontWeight: 700,
          color: 'var(--text)',
          lineHeight: 1.1,
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        {value}
        {unit && (
          <span
            style={{
              fontSize: '13px',
              color: 'var(--text-secondary)',
              fontWeight: 500,
              marginLeft: '4px',
            }}
          >
            {unit}
          </span>
        )}
        {trend && (
          <span
            style={{
              marginLeft: '6px',
              fontSize: '13px',
              color:
                trend === 'up' ? '#10b981' : trend === 'down' ? '#ef4444' : '#9ca3af',
            }}
          >
            {trend === 'up' ? '↑' : trend === 'down' ? '↓' : '→'}
          </span>
        )}
      </div>
      {helper && (
        <div
          style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '4px' }}
        >
          {helper}
        </div>
      )}
    </div>
  );
}

export function MetricGrid({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))',
        gap: '12px',
      }}
    >
      {children}
    </div>
  );
}

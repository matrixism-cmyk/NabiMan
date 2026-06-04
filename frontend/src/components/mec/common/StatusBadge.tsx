import React from 'react';

export type StatusTone =
  | 'success'
  | 'warning'
  | 'error'
  | 'info'
  | 'neutral'
  | 'running';

const TONE_STYLES: Record<StatusTone, React.CSSProperties> = {
  success: { background: '#d1fae5', color: '#065f46', borderColor: '#10b981' },
  warning: { background: '#fef3c7', color: '#92400e', borderColor: '#f59e0b' },
  error: { background: '#fee2e2', color: '#991b1b', borderColor: '#ef4444' },
  info: { background: '#dbeafe', color: '#1e40af', borderColor: '#3b82f6' },
  neutral: { background: '#f3f4f6', color: '#374151', borderColor: '#9ca3af' },
  running: { background: '#ede9fe', color: '#5b21b6', borderColor: '#8b5cf6' },
};

/// 문자열 값을 StatusTone 으로 매핑. 일관된 색상 체계를 전체 MEC 패널에
/// 제공하기 위한 헬퍼.
export function tone(value: string | boolean | undefined | null): StatusTone {
  if (value === true) return 'success';
  if (value === false) return 'error';
  const v = String(value ?? '').toLowerCase();
  switch (v) {
    case 'ready':
    case 'active':
    case 'connected':
    case 'running':
    case 'success':
    case 'pass':
    case 'bound':
    case 'assigned':
    case 'completed':
    case 'imported':
      return 'success';
    case 'pending':
    case 'provisioning':
    case 'warning':
    case 'partial':
    case 'unmanaged':
      return 'warning';
    case 'notready':
    case 'not_ready':
    case 'failed':
    case 'error':
    case 'terminating':
    case 'drop':
    case 'reject':
    case 'disconnected':
    case 'cancelled':
      return 'error';
    case 'unknown':
    case 'system_reserved':
    case 'available':
      return 'neutral';
    case 'info':
      return 'info';
    default:
      return 'neutral';
  }
}

/// Shape/symbol channel so severity never relies on color alone (CVD-safe,
/// WCAG). Pair with tone() color + a text label for the full 3 channels.
const TONE_GLYPHS: Record<StatusTone, string> = {
  success: '●',
  warning: '▲',
  error: '✖',
  info: 'ℹ',
  neutral: '■',
  running: '◐',
};

export function severityGlyph(value: StatusTone | string | boolean | null): string {
  const resolved: StatusTone =
    typeof value === 'string' && value in TONE_GLYPHS ? (value as StatusTone) : tone(value as any);
  return TONE_GLYPHS[resolved];
}

interface Props {
  children: React.ReactNode;
  tone?: StatusTone | string | boolean | null;
  size?: 'sm' | 'md';
  icon?: React.ReactNode;
  title?: string;
}

export default function StatusBadge({
  children,
  tone: toneProp,
  size = 'sm',
  icon,
  title,
}: Props) {
  const resolved: StatusTone =
    typeof toneProp === 'string' && toneProp in TONE_STYLES
      ? (toneProp as StatusTone)
      : tone(toneProp as any);

  const style: React.CSSProperties = {
    ...TONE_STYLES[resolved],
    display: 'inline-flex',
    alignItems: 'center',
    gap: '4px',
    padding: size === 'md' ? '3px 10px' : '2px 8px',
    borderRadius: '999px',
    fontSize: size === 'md' ? '12px' : '11px',
    fontWeight: 600,
    lineHeight: 1.3,
    border: '1px solid transparent',
    whiteSpace: 'nowrap',
    fontVariantNumeric: 'tabular-nums',
  };

  return (
    <span style={style} title={title}>
      {icon && <span style={{ display: 'inline-flex' }}>{icon}</span>}
      <span>{children}</span>
    </span>
  );
}

export function Dot({ tone: t }: { tone: StatusTone | string | boolean | null }) {
  const resolved: StatusTone =
    typeof t === 'string' && t in TONE_STYLES ? (t as StatusTone) : tone(t as any);
  return (
    <span
      style={{
        display: 'inline-block',
        width: '8px',
        height: '8px',
        borderRadius: '50%',
        background: TONE_STYLES[resolved].borderColor as string,
        marginRight: '6px',
      }}
    />
  );
}

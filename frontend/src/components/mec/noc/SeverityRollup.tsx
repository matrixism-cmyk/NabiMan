import React from 'react';
import { useT } from '../../../i18n';
import { useCountUp } from '../../../hooks/mec/useCountUp';
import { severityGlyph, StatusTone } from '../common';

interface Props {
  normal: number;
  caution: number;
  critical: number;
}

const CELLS: { key: keyof Props; tone: StatusTone; labelKey: string }[] = [
  { key: 'normal', tone: 'success', labelKey: 'noc.normal' },
  { key: 'caution', tone: 'warning', labelKey: 'noc.caution' },
  { key: 'critical', tone: 'error', labelKey: 'noc.critical' },
];

const COLOR: Record<StatusTone, string> = {
  success: '#10b981', warning: '#f59e0b', error: '#ef4444',
  info: '#3b82f6', neutral: '#9ca3af', running: '#8b5cf6',
};

function Cell({ value, tone, label }: { value: number; tone: StatusTone; label: string }) {
  const n = Math.round(useCountUp(value));
  const color = COLOR[tone];
  return (
    <span
      style={{ display: 'inline-flex', alignItems: 'center', gap: '5px', fontVariantNumeric: 'tabular-nums' }}
      title={label}
    >
      <span style={{ color, fontSize: '12px' }}>{severityGlyph(tone)}</span>
      <span style={{ color, fontWeight: 700, fontSize: '15px' }}>{n}</span>
      <span style={{ color: 'var(--text-secondary)', fontSize: '11px' }}>{label}</span>
    </span>
  );
}

/// Header 정상/주의/장애 triad — aggregate health at a glance, color + glyph + label.
export default function SeverityRollup({ normal, caution, critical }: Props) {
  const { t } = useT();
  const vals = { normal, caution, critical };
  return (
    <span style={{ display: 'inline-flex', alignItems: 'center', gap: '14px' }}>
      {CELLS.map((c) => (
        <Cell key={c.key} value={vals[c.key]} tone={c.tone} label={t(c.labelKey)} />
      ))}
    </span>
  );
}

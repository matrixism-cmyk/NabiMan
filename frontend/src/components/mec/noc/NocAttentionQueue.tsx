import React from 'react';
import { useT } from '../../../i18n';
import { NocSignal } from '../../../hooks/mec/useNocSignals';
import { severityGlyph } from '../common';

interface Props {
  items: NocSignal[];
  onPick?: (tab: string) => void;
}

const COLOR = { error: '#ef4444', warning: '#f59e0b' } as const;

/// Severity-sorted actionable worklist. Each row: glyph + color + text, with a
/// drill-down affordance into the relevant MEC panel.
export default function NocAttentionQueue({ items, onPick }: Props) {
  const { t } = useT();
  if (!items.length) {
    return (
      <div style={{ color: '#10b981', fontSize: '13px', display: 'flex', alignItems: 'center', gap: '6px', padding: '6px 2px' }}>
        <span>{severityGlyph('success')}</span>
        <span>{t('noc.attentionEmpty')}</span>
      </div>
    );
  }
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
      {items.map((s) => {
        const color = COLOR[s.tone as 'error' | 'warning'] || '#9ca3af';
        return (
          <button
            key={s.id}
            onClick={s.target && onPick ? () => onPick(s.target!) : undefined}
            className="mec-row-in"
            style={{
              display: 'flex', alignItems: 'center', gap: '8px', textAlign: 'left',
              background: 'var(--surface)', border: '1px solid var(--border)',
              borderLeft: `3px solid ${color}`, borderRadius: '6px', padding: '7px 10px',
              cursor: s.target && onPick ? 'pointer' : 'default', color: 'var(--text)',
            }}
          >
            <span style={{ color, fontSize: '12px' }}>{severityGlyph(s.tone)}</span>
            <span style={{ flex: 1, fontSize: '12px' }}>{s.text}</span>
            {s.target && onPick && <span style={{ color: 'var(--text-secondary)', fontSize: '11px' }}>→</span>}
          </button>
        );
      })}
    </div>
  );
}

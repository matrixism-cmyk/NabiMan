import React from 'react';
import { useT } from '../../../i18n';
import { NocSignal } from '../../../hooks/mec/useNocSignals';
import { severityGlyph } from '../common';

interface Props {
  items: NocSignal[];
  owner?: string;
}

const prefersReducedMotion = () =>
  typeof window !== 'undefined' &&
  window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

/// Bottom alert band. Scrolls critical/caution items with the on-call owner;
/// collapses to a calm "all normal" strip when clean; static under reduced-motion.
export default function NocAlertMarquee({ items, owner }: Props) {
  const { t } = useT();
  const alerts = items.filter((s) => s.severity === 'critical' || s.severity === 'caution');

  if (!alerts.length) {
    return (
      <div className="noc-marquee noc-marquee-ok">
        <span>{severityGlyph('success')} {t('noc.allHealthy')}</span>
      </div>
    );
  }

  const ownerSuffix = owner ? ` · ${t('noc.owner')}: ${owner}` : '';
  const chips = alerts.map((s) => {
    const color = s.tone === 'error' ? '#ef4444' : '#f59e0b';
    return (
      <span key={s.id} style={{ color, marginRight: '28px' }}>
        {severityGlyph(s.tone)} {s.text}
      </span>
    );
  });

  const hasCritical = alerts.some((s) => s.severity === 'critical');
  const cls = `noc-marquee ${hasCritical ? 'noc-marquee-crit' : 'noc-marquee-warn'}`;
  if (prefersReducedMotion()) {
    return <div className={cls} style={{ overflowX: 'auto' }}>{chips}<span style={{ color: 'var(--text-secondary)' }}>{ownerSuffix}</span></div>;
  }
  return (
    <div className={cls}>
      <div className="noc-marquee-track">
        {chips}
        <span style={{ color: 'var(--text-secondary)', marginRight: '28px' }}>{ownerSuffix}</span>
        {chips}
      </div>
    </div>
  );
}

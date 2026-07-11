import React, { useEffect, useState } from 'react';
import { useT } from '../../../i18n';

interface Props {
  /** Timestamp (ms) of the last successful data load. */
  lastUpdated: number | null;
  /** Refresh interval (ms), used to show a countdown ring to the next poll. */
  intervalMs?: number;
  /** True while a fetch is in flight. */
  refreshing?: boolean;
}

/// "LIVE" pill with a pulsing dot and a self-ticking "n초 전 갱신" label so the
/// dashboard reads as live even between polls.
export default function LiveIndicator({ lastUpdated, intervalMs, refreshing }: Props) {
  const { t } = useT();
  const agoText = (secs: number): string => {
    if (secs < 2) return t('mec.live.justNow');
    if (secs < 60) return t('mec.live.secsAgo', { s: secs });
    return t('mec.live.minsAgo', { m: Math.floor(secs / 60) });
  };
  const [, force] = useState(0);
  useEffect(() => {
    const id = setInterval(() => force((x) => x + 1), 1000);
    return () => clearInterval(id);
  }, []);

  const secs = lastUpdated ? Math.max(0, Math.floor((Date.now() - lastUpdated) / 1000)) : 0;
  const stale = intervalMs ? secs * 1000 > intervalMs * 2 : false;
  const color = refreshing ? '#3b82f6' : stale ? '#f59e0b' : '#10b981';

  return (
    <span
      title={lastUpdated ? new Date(lastUpdated).toLocaleString() : undefined}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: '6px',
        fontSize: '12px',
        color: 'var(--text-secondary)',
        userSelect: 'none',
      }}
    >
      <span
        className="mec-live-dot"
        style={{
          width: '8px',
          height: '8px',
          borderRadius: '50%',
          background: color,
          boxShadow: `0 0 0 0 ${color}`,
        }}
      />
      <span style={{ fontWeight: 600, color, letterSpacing: '0.04em' }}>LIVE</span>
      <span>· {lastUpdated ? agoText(secs) : t('mec.live.connecting')}</span>
    </span>
  );
}

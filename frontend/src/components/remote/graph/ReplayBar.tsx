import React from 'react';
import { useT } from '../../../shared/i18n';
import { REPLAY_SPEEDS } from './view/graphSettings';
import type { ReplayState } from './view/useReplay';

interface Props { state: ReplayState; speed: number; onSpeed: (speed: number) => void; onStop: () => void }

/** The time-lapse's clock: the time of the record it has reached, how far through, its speed, and a stop. */
export default function ReplayBar({ state, speed, onSpeed, onStop }: Props) {
  const { t } = useT();
  const when = new Date(state.clock * 1000).toLocaleString([], { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false });
  const pct = state.total ? Math.round((state.done / state.total) * 100) : 0;
  return (
    <div className="graph-replay-bar" role="status" data-testid="graph-replay-bar">
      <span className="graph-replay-dot" aria-hidden="true" />
      <time className="graph-replay-clock">{when}</time>
      <span className="graph-replay-track" aria-hidden="true"><i style={{ width: `${pct}%` }} /></span>
      <span className="graph-replay-count">{state.done}/{state.total}</span>
      <select value={speed} aria-label={t('graph.replay.speed')} onChange={(e) => onSpeed(Number(e.target.value))}>
        {REPLAY_SPEEDS.map((v) => <option key={v} value={v}>{t(`graph.replay.speed.${v}`)}</option>)}
      </select>
      <button type="button" className="btn btn-secondary btn-sm" onClick={onStop}>■ {t('graph.replay.stop')}</button>
      <span className="graph-replay-hint">{t('graph.replay.stopHint')}</span>
    </div>
  );
}

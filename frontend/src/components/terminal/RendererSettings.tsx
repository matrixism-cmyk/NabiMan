import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import { RendererChoice, rendererStates } from './renderers';
import { benchmarkRenderers, BenchResult } from './rendererBench';
import { fontOf } from './terminalFonts';

const NAMES: Record<string, string> = { webgl: 'WebGL', canvas: 'Canvas', dom: 'DOM' };

/**
 * The drawing method: chosen per browser, shown for the windows open now, and
 * measured on request. See ./renderers for why WebGL, then Canvas, then DOM.
 */
export default function RendererSettings({ value, onChange, fontFamily, fontSize }: {
  value: RendererChoice;
  onChange: (v: RendererChoice) => void;
  fontFamily: string;
  fontSize: number;
}) {
  const { t } = useT();
  const [bench, setBench] = useState<BenchResult[] | null>(null);
  const [running, setRunning] = useState(false);

  const now = rendererStates()
    .map((s) => `${NAMES[s.active]}${s.reason ? ` (${t(`terminal.rendererReason.${s.reason}`)})` : ''}`)
    .reduce<Record<string, number>>((acc, k) => ({ ...acc, [k]: (acc[k] ?? 0) + 1 }), {});
  const nowText = Object.entries(now).map(([k, n]) => (n > 1 ? `${k} ×${n}` : k)).join(' · ');

  const run = async () => {
    setRunning(true);
    try {
      setBench(await benchmarkRenderers(fontOf(fontFamily).family, fontSize));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="form-group">
      <label>{t('terminal.renderer')}</label>
      <div className="filter-row">
        {(['auto', 'webgl', 'canvas', 'dom'] as RendererChoice[]).map((c) => (
          <button
            key={c} type="button"
            className={`btn btn-sm ${value === c ? 'btn-primary' : 'btn-secondary'}`}
            onClick={() => onChange(c)}
          >{c === 'auto' ? t('terminal.rendererAuto') : NAMES[c]}</button>
        ))}
      </div>
      <small className="text-secondary">{t('terminal.rendererHint')}</small>
      <div className="text-secondary">{t('terminal.rendererNow')}: {nowText || t('terminal.rendererNone')}</div>
      <div className="filter-row">
        <button type="button" className="btn btn-sm btn-secondary" onClick={run} disabled={running}>
          {running ? t('terminal.rendererBenchRunning') : t('terminal.rendererBench')}
        </button>
        {bench && (
          <span data-testid="renderer-bench">
            {bench.map((b) => {
              const label = b.active && b.active !== b.requested ? `${NAMES[b.requested]}→${NAMES[b.active]}` : NAMES[b.requested];
              const fps = b.fps === null ? '–' : `${Math.round(b.fps)}fps`;
              return `${label} ${fps}${b.cpuMs === null ? '' : ` (CPU ${b.cpuMs.toFixed(1)}ms)`}`;
            }).join(' · ')}
          </span>
        )}
      </div>
      {bench && <small className="text-secondary">{t('terminal.rendererBenchNote')}</small>}
    </div>
  );
}

import React from 'react';
import { useT } from '../../i18n';
import { formatDateTime } from '../../shared/i18n';
import type { RackLayout, RackStatus } from './rackTypes';
import { CALM_GLYPH, LEVELS, LEVEL_GLYPH, LOOKS, celsius } from './rackFormat';

/** The numbers to take in first: nodes ready, GPUs, warnings, the hottest reading, and how many of the other devices answer. */
export function RackSummary({ status }: { status: RackStatus }) {
  const { t } = useT();
  const s = status.summary;
  const known = status.source === 'ok' || status.source === 'partial';
  const tile = (key: string, label: string, value: string, tone: string) => (
    <div className={`rack-tile tone-${tone}`} data-tile={key}>
      <span className="rack-tile-label">{label}</span>
      <span className="rack-tile-value">{value}</span>
    </div>
  );
  const hot = s.max_temp_c !== null && s.max_temp_c >= status.thresholds.temp_c;
  return (
    <div className="rack-summary" role="group" aria-label={t('rack.title')}>
      {tile('nodes', t('rack.sum.nodes'), known ? `${s.nodes_ready}/${s.nodes_total}` : `–/${s.nodes_total}`, !known ? 'idle' : s.nodes_ready === s.nodes_total ? 'ok' : 'down')}
      {tile('gpus', t('rack.sum.gpus'), known ? t('rack.sum.gpusUnit', { count: s.gpus }) : '–', 'plain')}
      {tile('warnings', t('rack.sum.warnings'), known ? `${s.warnings > 0 ? '▲ ' : ''}${t('rack.sum.count', { count: s.warnings })}` : '–', s.warnings > 0 ? 'warn' : 'plain')}
      {s.down > 0 && tile('down', t('rack.sum.down'), `✖ ${t('rack.sum.count', { count: s.down })}`, 'down')}
      {s.probed > 0 && tile('reach', t('rack.sum.reach'), `${s.noreply > 0 ? `${LEVEL_GLYPH.noreply} ` : ''}${s.reachable}/${s.probed}`, s.noreply > 0 ? 'down' : s.reachable === s.probed ? 'ok' : 'plain')}
      {tile('maxTemp', t('rack.sum.maxTemp'), `${hot ? '▲ ' : ''}${celsius(s.max_temp_c)}`, hot ? 'warn' : 'plain')}
    </div>
  );
}

/** What the colours, marks and outlines mean. */
export function RackLegend({ status, tentative }: { status: RackStatus | null; tentative: boolean }) {
  const { t } = useT();
  return (
    <div className="rack-legend" aria-label={t('rack.legend')}>
      <ul>
        {LOOKS.map((look) => <li key={look}><span className={`rack-swatch look-${look}`} aria-hidden="true" />{t(`rack.look.${look}`)}</li>)}
      </ul>
      <ul>
        {LEVELS.map((level) => <li key={level}><span className={`rack-light level-${level}`} aria-hidden="true">{LEVEL_GLYPH[level]}</span>{t(`rack.legend.level.${level}`)}</li>)}
        <li><span className="rack-light level-noreply is-calm" aria-hidden="true">{CALM_GLYPH}</span>{t('rack.level.externalNoreply')}</li>
        {tentative && <li><span className="rack-swatch is-dashed" aria-hidden="true" />{t('rack.legend.dashed')}</li>}
      </ul>
      {status && (
        <p>{t('rack.legend.warnRule', { temp: status.thresholds.temp_c, mem: status.thresholds.memory_pct, disk: status.thresholds.disk_pct })}</p>
      )}
    </div>
  );
}

/** The layout's version, when status was last collected, and (folded away) where the layout comes from and the power feeds. */
export function RackMeta({ layout, status, tentative }: { layout: RackLayout; status: RackStatus | null; tentative: boolean }) {
  const { t } = useT();
  return (
    <div className="rack-meta">
      <span>{t('rack.version', { version: layout.version || '–' })}</span>
      {status && <span data-updated>{t('rack.updated', { time: formatDateTime(status.collected_at, { dateStyle: 'short', timeStyle: 'medium' }) })}</span>}
      <span>{t('rack.autoRefresh')}</span>
      {(layout.sources.length > 0 || layout.notes) && (
        <details>
          <summary>{t('rack.sources', { count: layout.sources.length })}</summary>
          <ul>{layout.sources.map((source) => <li key={source}>{source}</li>)}</ul>
          {layout.notes && <p><strong>{t('rack.notes')}</strong> {layout.notes}</p>}
        </details>
      )}
      {layout.power.length > 0 && (
        <details>
          <summary>{t('rack.power')}</summary>
          <dl className="rack-facts">
            {layout.power.map((p) => <React.Fragment key={p.label}><dt>{p.label}</dt><dd>{p.value}</dd></React.Fragment>)}
          </dl>
        </details>
      )}
      {tentative && <span className="rack-tentative" data-tentative>{t('rack.tentative')}</span>}
    </div>
  );
}

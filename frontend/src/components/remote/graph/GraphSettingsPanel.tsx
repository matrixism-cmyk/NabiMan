import React from 'react';
import { useT } from '../../../shared/i18n';
import { FORCE_DEFAULTS, GraphSettings, RANGES, SectionKey } from './view/graphSettings';
import './view/graph-settings.css';

interface Props {
  settings: GraphSettings;
  onChange: (patch: Partial<GraphSettings>) => void;
  search: string;
  onSearch: (q: string) => void;
  /** How many nodes the search found. */
  matches: number;
  /** Enter in the search box: fly to the first match. */
  onSearchEnter: () => void;
  /** The servers and their colours, for the legend. */
  groups: [string, string][];
  /** How many network devices are entered (the toggle shows only with some). */
  devices: number;
  /** The network tiers hold the heights. */
  tiered: boolean;
}

type Slider = keyof typeof RANGES;

/** The settings at the map's right, in Obsidian's sections: filters, groups, display, forces. */
export default function GraphSettingsPanel({ settings: s, onChange, search, onSearch, matches, onSearchEnter, groups, devices, tiered }: Props) {
  const { t } = useT();
  const section = (key: SectionKey, children: React.ReactNode) => (
    <section className="graph-settings-section">
      <button type="button" className="graph-settings-head" aria-expanded={s.sections[key]} data-section={key}
        onClick={() => onChange({ sections: { ...s.sections, [key]: !s.sections[key] } })}>
        <span aria-hidden="true">{s.sections[key] ? '▾' : '▸'}</span> {t(`graph.settings.${key}`)}
      </button>
      {s.sections[key] && <div className="graph-settings-body">{children}</div>}
    </section>
  );
  const toggle = (key: 'showDevices' | 'showEnded' | 'showServers' | 'showOrphans' | 'colorGroups' | 'arrows' | 'autoRotate', text: string) => (
    <label className="graph-settings-toggle">
      <span>{text}</span>
      <input type="checkbox" role="switch" checked={s[key]} data-setting={key} onChange={(e) => onChange({ [key]: e.target.checked })} />
    </label>
  );
  const slider = (key: Slider, text: string, step = 0.05) => (
    <label className="graph-settings-slider">
      <span>{text}</span>
      <input type="range" min={RANGES[key][0]} max={RANGES[key][1]} step={step} value={s[key]} data-setting={key}
        onChange={(e) => onChange({ [key]: Number(e.target.value) })} />
    </label>
  );
  return (
    <aside className="graph-settings" aria-label={t('graph.settings')} data-testid="graph-settings">
      {section('filters', (
        <>
          <input type="search" className="graph-settings-search" value={search} placeholder={t('graph.search.placeholder')} aria-label={t('graph.search')}
            data-testid="graph-search" onChange={(e) => onSearch(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter') onSearchEnter(); if (e.key === 'Escape') onSearch(''); }} />
          {search.trim() && <p className="graph-settings-note" aria-live="polite">{matches ? t('graph.search.count', { count: matches }) : t('graph.search.none')}</p>}
          {devices > 0 && toggle('showDevices', `${t('graph.filter.devices')} (${devices})`)}
          {toggle('showServers', t('graph.filter.servers'))}
          {toggle('showEnded', t('graph.filter.ended'))}
          {toggle('showOrphans', t('graph.filter.orphans'))}
        </>
      ))}
      {section('groups', (
        <>
          {toggle('colorGroups', t('graph.groups.color'))}
          <ul className="graph-settings-legend">
            <li className="is-lead">★ {t('graph.legend.lead')}</li>
            <li className="is-busy">● {t('graph.legend.busy')}</li>
            {groups.map(([g, color]) => (
              <li key={g}><i style={{ background: s.colorGroups ? color : undefined }} />{g === 'local' ? t('graph.legend.local') : g}</li>
            ))}
          </ul>
        </>
      ))}
      {section('display', (
        <>
          <div className="btn-group graph-settings-style" role="group" aria-label={t('graph.style')}>
            {(['simple', 'rich'] as const).map((v) => (
              <button key={v} type="button" data-style={v} className={`btn btn-sm ${s.style === v ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={s.style === v}
                onClick={() => onChange({ style: v })}>{t(`graph.style.${v}`)}</button>
            ))}
          </div>
          {toggle('arrows', t('graph.display.arrows'))}
          {toggle('autoRotate', `⟳ ${t('graph.rotate')}`)}
          {slider('labelThreshold', t('graph.display.labels'))}
          {s.style === 'simple' && <p className="graph-settings-note">{t('graph.display.labelsHint')}</p>}
          {slider('nodeSize', t('graph.display.nodeSize'))}
          {slider('linkThickness', t('graph.display.linkThickness'))}
        </>
      ))}
      {section('forces', (
        <>
          {slider('center', t('graph.force.center'))}
          {slider('repel', t('graph.force.repel'))}
          {slider('linkStrength', t('graph.force.link'))}
          {slider('linkDistance', t('graph.force.distance'))}
          {tiered && <p className="graph-settings-note">{t('graph.force.tiered')}</p>}
          <button type="button" className="btn btn-secondary btn-sm" data-testid="graph-forces-reset" onClick={() => onChange({ ...FORCE_DEFAULTS })}>{t('graph.force.reset')}</button>
        </>
      ))}
    </aside>
  );
}

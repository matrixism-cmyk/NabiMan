import React, { Suspense, useMemo, useState } from 'react';
import { useT } from '../../../shared/i18n';
import ClaudeAddress from '../../terminal/ClaudeAddress';
import type { SessionLink } from '../SessionLinks';
import type { SessionInfo } from '../SessionsView';
import { buildGraph, ViewMode } from './graphModel';
import type { Layout2D } from './Graph2D';
import './session-graph.css';

// The graph libraries are large: fetched only when a graph is asked for.
const Graph2D = React.lazy(() => import('./Graph2D'));
const Graph3D = React.lazy(() => import('./Graph3D'));

const LAYOUTS: Layout2D[] = ['concentric', 'tree', 'free'];
const LAYOUT_KEY = 'nabiman_graph2d_layout';
const ROTATE_KEY = 'nabiman_graph3d_rotate';
const stored = (key: string, fallback: string) => { try { return localStorage.getItem(key) || fallback; } catch { return fallback; } };
const store = (key: string, value: string) => { try { localStorage.setItem(key, value); } catch { /* private mode */ } };

interface Props {
  mode: Exclude<ViewMode, 'tiles'>;
  sessions: SessionInfo[];
  links: SessionLink[];
  onAttach: (session: SessionInfo) => void;
  onPopOut: (session: SessionInfo) => void;
}

/** WebGL, which the 3D view needs; without it the 2D view is offered instead. */
function hasWebGL() {
  try {
    const c = document.createElement('canvas');
    return !!(c.getContext('webgl2') || c.getContext('webgl'));
  } catch {
    return false;
  }
}

/** The sessions drawn as a graph (2D or 3D), with a panel for the one picked. */
export default function SessionGraphView({ mode, sessions, links, onAttach, onPopOut }: Props) {
  const { t } = useT();
  const graph = useMemo(() => buildGraph(sessions, links), [sessions, links]);
  const [selected, setSelected] = useState<string | null>(null);
  const [tip, setTip] = useState<{ note: string; x: number; y: number } | null>(null);
  const [layout, setLayout] = useState<Layout2D>(() => stored(LAYOUT_KEY, 'concentric') as Layout2D);
  const [rotate, setRotate] = useState(() => stored(ROTATE_KEY, '1') === '1');
  const picked = sessions.find((s) => s.id === selected) || null;
  const groups = Array.from(new Map(graph.nodes.map((n) => [n.group, n.color])).entries());
  const onHoverEdge = (note: string | null, x: number, y: number) => setTip(note ? { note, x, y } : null);
  const webgl = mode !== 'graph3d' || hasWebGL();
  const incoming = picked ? links.filter((l) => l.to === picked.id) : [];
  const outgoing = picked ? links.filter((l) => l.from === picked.id) : [];
  const nameOf = (id: string) => { const s = sessions.find((x) => x.id === id); return s?.claude?.[0]?.name || s?.title || s?.label || id; };

  return (
    <div className={`session-graph ${mode === 'graph3d' ? 'is-3d' : ''}`}>
      <div className="session-graph-bar">
        {mode === 'graph2d' && (
          <div className="btn-group" role="group" aria-label={t('graph.layout')}>
            {LAYOUTS.map((l) => (
              <button key={l} type="button" className={`btn btn-sm ${layout === l ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={layout === l}
                onClick={() => { setLayout(l); store(LAYOUT_KEY, l); }}>{t(`graph.layout.${l}`)}</button>
            ))}
          </div>
        )}
        {mode === 'graph3d' && (
          <button type="button" className={`btn btn-sm ${rotate ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={rotate}
            onClick={() => { setRotate(!rotate); store(ROTATE_KEY, rotate ? '0' : '1'); }}>⟳ {t('graph.rotate')}</button>
        )}
        <div className="session-graph-legend">
          <span className="session-graph-key is-lead">★ {t('graph.legend.lead')}</span>
          <span className="session-graph-key is-busy">● {t('graph.legend.busy')}</span>
          {groups.map(([g, color]) => (
            <span key={g} className="session-graph-key"><i style={{ background: color }} />{g === 'local' ? t('graph.legend.local') : g}</span>
          ))}
          <span className="session-graph-count">{t('graph.count', { sessions: graph.nodes.length, links: graph.edges.length })}</span>
        </div>
      </div>
      <div className="session-graph-stage">
        {webgl ? (
          <Suspense fallback={<p className="session-graph-wait">{t('graph.loading')}</p>}>
            {mode === 'graph2d'
              ? <Graph2D graph={graph} layout={layout} selected={selected} onSelect={setSelected} onHoverEdge={onHoverEdge} />
              : <Graph3D graph={graph} selected={selected} onSelect={setSelected} onHoverEdge={onHoverEdge} autoRotate={rotate} />}
          </Suspense>
        ) : <p className="session-graph-wait" role="alert">{t('graph.noWebgl')}</p>}
        <p className="session-graph-hint">{t(mode === 'graph2d' ? 'graph.hint2d' : 'graph.hint3d')}</p>
        {picked && (
          <aside className="session-graph-panel" aria-label={picked.title || picked.label}>
            <div className="session-graph-panel-head">
              <strong>{picked.lead ? '★ ' : ''}{picked.title || picked.label}</strong>
              <button type="button" className="btn btn-secondary btn-sm" onClick={() => setSelected(null)} aria-label={t('graph.close')}>✕</button>
            </div>
            <code className="session-label">{picked.label}</code>
            <div className="session-graph-badges">
              {picked.lead && <span className="session-work-badge is-orchestrator">{t('claudePeer.lead', { count: outgoing.length })}</span>}
              {picked.busy && <span className="session-work-badge">{t('claudePeer.working')}</span>}
              {!picked.alive && <span className="session-work-badge is-still">{t('graph.ended')}</span>}
            </div>
            <ClaudeAddress variant="card" peers={picked.claude || []} />
            {(incoming.length > 0 || outgoing.length > 0) && (
              <ul className="session-graph-rel">
                {incoming.map((l) => <li key={l.from}><span className="is-in">←</span> {nameOf(l.from)}</li>)}
                {outgoing.map((l) => <li key={l.to}><span className="is-out">→</span> {nameOf(l.to)}</li>)}
              </ul>
            )}
            <pre className="session-graph-preview">{picked.preview || t('graph.noPreview')}</pre>
            <div className="session-actions">
              <button className="btn btn-primary btn-sm" disabled={!picked.alive} onClick={() => onAttach(picked)}>{t('terminal.join')}</button>
              <button className="btn btn-secondary btn-sm" disabled={!picked.alive} onClick={() => onPopOut(picked)} title={t('remote.detachHint')}>⧉</button>
            </div>
          </aside>
        )}
      </div>
      {tip && (
        <div className="session-link-tip session-graph-tip" role="tooltip" style={{ left: tip.x + 14, top: tip.y + 16 }}>
          {tip.note.split('\n').map((line, i) => <div key={i} className={i === 0 ? 'session-link-tip-head' : 'session-link-tip-line'}>{line}</div>)}
        </div>
      )}
    </div>
  );
}

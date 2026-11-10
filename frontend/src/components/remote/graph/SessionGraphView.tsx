import React, { Suspense, useEffect, useMemo, useRef, useState } from 'react';
import { apiRequest } from '../../../shared/api/useApi';
import { useT } from '../../../shared/i18n';
import ClaudeAddress from '../../terminal/ClaudeAddress';
import type { SessionLink } from '../SessionLinks';
import type { SessionInfo } from '../SessionsView';
import { buildGraph, buildWorkMap, MapEvent, MapServer, ServerFacts, Topology, ViewMode } from './graphModel';
import type { Layout2D } from './Graph2D';
import TopologyEditor from './TopologyEditor';
import AgentIcons from './AgentIcons';
import AttackPanel from './AttackPanel';
import './session-graph.css';
import { startVisibleInterval } from '../../../shared/ui/useVisibleInterval';

// The graph libraries are large: fetched only when a graph is asked for.
const Graph2D = React.lazy(() => import('./Graph2D'));
const Graph3D = React.lazy(() => import('./Graph3D'));

const LAYOUTS: Layout2D[] = ['concentric', 'tree', 'free'];
const LAYOUT_KEY = 'nabiman_graph2d_layout';
const ROTATE_KEY = 'nabiman_graph3d_rotate';
const NETWORK_KEY = 'nabiman_graph3d_network';
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
  // The work map (3D): the machines the sessions run on, and the messages between sessions as they happen.
  const [servers, setServers] = useState<MapServer[]>([]);
  const [events, setEvents] = useState<MapEvent[]>([]);
  const newest = useRef(0);
  const [topology, setTopology] = useState<Topology | null>(null);
  const [editing, setEditing] = useState(false);
  const [facts, setFacts] = useState<ServerFacts[]>([]);
  const [checking, setChecking] = useState(false);
  // A request that fails outright (offline, server down) is said above the map, which would otherwise just go stale.
  const [problem, setProblem] = useState('');
  const fail = (e: unknown) => setProblem(t('graph.loadFailed', { message: e instanceof Error ? e.message : String(e) }));
  const loadFacts = () => apiRequest('/api/remote-servers/facts').then((r: any) => { if (r?.success && Array.isArray(r.data)) setFacts(r.data); }).catch(fail);
  // Asks every saved server how it is (the same check as 「전체 확인」), then shows what they said.
  const checkAll = async () => {
    setChecking(true);
    try {
      await apiRequest('/api/remote-servers/check-all', { method: 'POST' });
      const r: any = await apiRequest('/api/remote-servers');
      if (r?.success && Array.isArray(r.data)) setServers(r.data);
      await loadFacts();
    } catch (e) { setProblem(t('graph.checkFailed', { message: e instanceof Error ? e.message : String(e) })); } finally { setChecking(false); }
  };
  const [network, setNetwork] = useState(() => stored(NETWORK_KEY, '1') === '1');
  useEffect(() => {
    if (mode !== 'graph3d') return undefined;
    let live = true;
    void loadFacts();
    apiRequest('/api/terminal/topology').then((r: any) => { if (live && r?.success && r.data?.devices) setTopology(r.data); }).catch((e) => { if (live) fail(e); });
    apiRequest('/api/remote-servers').then((r: any) => { if (live && r?.success && Array.isArray(r.data)) setServers(r.data); }).catch((e) => { if (live) fail(e); });
    const pull = () => apiRequest(`/api/terminal/claude-events?window=3600&after=${newest.current}`).then((r: any) => {
      if (live && r?.success) setProblem('');
      if (!live || !r?.success || !Array.isArray(r.data) || !r.data.length) return;
      newest.current = r.data[r.data.length - 1].id;
      setEvents((old) => [...old, ...r.data].slice(-300));
    }).catch((e) => { if (live) fail(e); });
    void pull();
    const stopPulling = startVisibleInterval(() => { void pull(); }, 3000);
    return () => { live = false; stopPulling(); };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode]);
  const graph = useMemo(
    () => (mode === 'graph3d' ? buildWorkMap(sessions, links, servers, events, t('graph.legend.local'), network ? topology : null, facts) : buildGraph(sessions, links)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [sessions, links, servers, events, mode, topology, network, facts],
  );
  const [selected, setSelected] = useState<string | null>(null);
  const [tip, setTip] = useState<{ note: string; x: number; y: number } | null>(null);
  const [layout, setLayout] = useState<Layout2D>(() => stored(LAYOUT_KEY, 'concentric') as Layout2D);
  const [rotate, setRotate] = useState(() => stored(ROTATE_KEY, '1') === '1');
  const picked = sessions.find((s) => s.id === selected) || null;
  const machine = mode === 'graph3d' ? graph.nodes.find((n) => n.id === selected && n.kind === 'server') || null : null;
  // The saved server behind a machine node (`local` for this one); none for a host that is not saved.
  const machineId = machine ? (machine.id === 'host:local' ? 'local' : machine.id.startsWith('srv:') ? machine.id.slice(4) : null) : null;
  const groups = Array.from(new Map(graph.nodes.filter((n) => n.kind !== 'device').map((n) => [n.group, n.color])).entries());
  const clock = (at: number) => new Date(at * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
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
        {mode === 'graph3d' && !!topology?.devices.length && (
          <button type="button" className={`btn btn-sm ${network ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={network}
            onClick={() => { setNetwork(!network); store(NETWORK_KEY, network ? '0' : '1'); }}>⛨ {t('graph.network', { count: topology.devices.length })}</button>
        )}
        {mode === 'graph3d' && (
          <button type="button" className="btn btn-secondary btn-sm" disabled={checking} onClick={() => void checkAll()} title={t('graph.checkHint')}>⟳ {t(checking ? 'graph.checking' : 'graph.check')}</button>
        )}
        {mode === 'graph3d' && (
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => setEditing(true)} title={t('topology.note')}>✎ {t('topology.edit')}</button>
        )}
        <div className="session-graph-legend">
          <span className="session-graph-key is-lead">★ {t('graph.legend.lead')}</span>
          <span className="session-graph-key is-busy">● {t('graph.legend.busy')}</span>
          {groups.map(([g, color]) => (
            <span key={g} className="session-graph-key"><i style={{ background: color }} />{g === 'local' ? t('graph.legend.local') : g}</span>
          ))}
          <span className="session-graph-count">{t('graph.count', { sessions: sessions.length, links: links.length })}</span>
        </div>
      </div>
      {problem && <div role="alert" className="message" onClick={() => setProblem('')}>{problem}</div>}
      <div className="session-graph-stage">
        {webgl ? (
          <Suspense fallback={<p className="session-graph-wait">{t('graph.loading')}</p>}>
            {mode === 'graph2d'
              ? <Graph2D graph={graph} layout={layout} selected={selected} onSelect={setSelected} onHoverEdge={onHoverEdge} />
              : <Graph3D graph={graph} selected={selected} onSelect={setSelected} onHoverEdge={onHoverEdge} autoRotate={rotate} events={events} />}
          </Suspense>
        ) : <p className="session-graph-wait" role="alert">{t('graph.noWebgl')}</p>}
        <p className="session-graph-hint">{t(mode === 'graph2d' ? 'graph.hint2d' : 'graph.hint3d')}</p>
        {machine && <AttackPanel id={machineId} name={machine.name} sub={machine.sub} onClose={() => setSelected(null)} />}
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
            <AgentIcons kinds={picked.agents} claude={picked.claude?.[0]} />
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
      {mode === 'graph3d' && events.length > 0 && (
        <ol className="session-graph-timeline" aria-label={t('graph.timeline')}>
          {events.slice(-40).reverse().map((e) => (
            <li key={e.id}>
              <button type="button" onClick={() => setSelected(e.to || e.from)} disabled={!e.to && !e.from} title={e.text}>
                <time>{clock(e.at)}</time>
                <b>{e.from_name || '?'}</b><span aria-hidden="true"> → </span><b>{e.to_name || '?'}</b>
                <span className="session-graph-timeline-text">{e.text}</span>
              </button>
            </li>
          ))}
        </ol>
      )}
      {editing && (
        <TopologyEditor topology={topology || { devices: [], links: [], hosts: [] }} serverHosts={['localhost', ...servers.map((x) => x.host)]}
          onSaved={setTopology} onClose={() => setEditing(false)} />
      )}
      {tip && (
        <div className="session-link-tip session-graph-tip" role="tooltip" style={{ left: tip.x + 14, top: tip.y + 16 }}>
          {tip.note.split('\n').map((line, i) => <div key={i} className={i === 0 ? 'session-link-tip-head' : 'session-link-tip-line'}>{line}</div>)}
        </div>
      )}
    </div>
  );
}

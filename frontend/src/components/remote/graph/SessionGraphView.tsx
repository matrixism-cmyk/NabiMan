import React, { Suspense, useEffect, useMemo, useRef, useState } from 'react';
import { apiRequest } from '../../../shared/api/useApi';
import { useT } from '../../../shared/i18n';
import type { SessionLink } from '../SessionLinks';
import type { SessionInfo } from '../SessionsView';
import { buildGraph, buildWorkMap, MapEvent, MapServer, ServerFacts, Topology, ViewMode } from './graphModel';
import type { Layout2D } from './Graph2D';
import TopologyEditor from './TopologyEditor';
import AttackPanel from './AttackPanel';
import SessionPanel from './SessionPanel';
import GraphSettingsPanel from './GraphSettingsPanel';
import ReplayBar from './ReplayBar';
import { GraphSettings, loadSettings, saveSettings } from './view/graphSettings';
import { filterGraph, localSubset, searchMatches } from './view/graphFocus';
import { useLightTheme, useReducedMotion } from './view/graphTheme';
import { REPLAY_WINDOW } from './view/graphReplay';
import { useReplay } from './view/useReplay';
import './session-graph.css';
import { startVisibleInterval } from '../../../shared/ui/useVisibleInterval';

// The graph libraries are large: fetched only when a graph is asked for.
const Graph2D = React.lazy(() => import('./Graph2D'));
const Graph3D = React.lazy(() => import('./Graph3D'));

const LAYOUTS: Layout2D[] = ['concentric', 'tree', 'free'];
const LAYOUT_KEY = 'nabiman_graph2d_layout';
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
  // The 3D map's settings panel (as in Obsidian), its search, the local graph and the time-lapse.
  const [settings, setSettings] = useState<GraphSettings>(() => loadSettings());
  const change = (patch: Partial<GraphSettings>) => setSettings((old) => { const next = { ...old, ...patch }; saveSettings(next); return next; });
  const [search, setSearch] = useState('');
  const [flyTo, setFlyTo] = useState<{ id: string; seq: number } | null>(null);
  const [local, setLocal] = useState<{ id: string; depth: number } | null>(null);
  const [bursts, setBursts] = useState<{ seq: number; list: MapEvent[] } | null>(null);
  const replay = useReplay((list) => setBursts((b) => ({ seq: (b?.seq || 0) + 1, list })));
  const [replayNote, setReplayNote] = useState('');
  const light = useLightTheme();
  const reducedMotion = useReducedMotion();
  const network = settings.showDevices;
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
  const full = useMemo(
    () => (mode === 'graph3d' ? buildWorkMap(sessions, links, servers, events, t('graph.legend.local'), network ? topology : null, facts) : buildGraph(sessions, links)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [sessions, links, servers, events, mode, topology, network, facts],
  );
  const { showDevices, showEnded, showServers, showOrphans } = settings;
  const graph = useMemo(() => {
    if (mode !== 'graph3d') return full;
    const shown = filterGraph(full, { showDevices, showEnded, showServers, showOrphans });
    return local ? localSubset(shown, local.id, local.depth) : shown;
  }, [full, mode, showDevices, showEnded, showServers, showOrphans, local]);
  const matches = useMemo(() => (mode === 'graph3d' ? searchMatches(graph.nodes, search) : []), [graph, search, mode]);
  // The time-lapse: the last day's record, fetched when asked for (the live feed keeps only the last hour).
  const startReplay = async () => {
    setReplayNote(t('graph.replay.loading'));
    try {
      const r: any = await apiRequest(`/api/terminal/claude-events?window=${REPLAY_WINDOW}&after=0`);
      const day: MapEvent[] = r?.success && Array.isArray(r.data) ? r.data : [];
      setReplayNote(replay.start([...day, ...events], settings.replaySpeed) ? '' : t('graph.replay.empty'));
    } catch (e) { setReplayNote(t('graph.replay.failed', { message: e instanceof Error ? e.message : String(e) })); }
  };
  const stopReplay = () => { if (replay.state.playing) replay.stop(); };
  useEffect(() => {
    if (!replay.state.playing) return undefined;
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') replay.stop(); };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [replay.state.playing]);
  const [selected, setSelected] = useState<string | null>(null);
  const [tip, setTip] = useState<{ note: string; x: number; y: number } | null>(null);
  const [layout, setLayout] = useState<Layout2D>(() => stored(LAYOUT_KEY, 'concentric') as Layout2D);
  const picked = sessions.find((s) => s.id === selected) || null;
  const machine = mode === 'graph3d' ? graph.nodes.find((n) => n.id === selected && n.kind === 'server') || null : null;
  // The saved server behind a machine node (`local` for this one); none for a host that is not saved.
  const machineId = machine ? (machine.id === 'host:local' ? 'local' : machine.id.startsWith('srv:') ? machine.id.slice(4) : null) : null;
  const groups = Array.from(new Map(graph.nodes.filter((n) => n.kind !== 'device').map((n) => [n.group, n.color])).entries());
  const clock = (at: number) => new Date(at * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hour12: false });
  const onHoverEdge = (note: string | null, x: number, y: number) => setTip(note ? { note, x, y } : null);
  const webgl = mode !== 'graph3d' || hasWebGL();
  const is3d = mode === 'graph3d';
  const localName = local ? graph.nodes.find((n) => n.id === local.id)?.name || full.nodes.find((n) => n.id === local.id)?.name || local.id : '';

  return (
    <div className={`session-graph ${is3d ? `is-3d is-${settings.style}` : ''} ${is3d && light && settings.style === 'simple' ? 'is-light' : ''}`}>
      <div className="session-graph-bar">
        {mode === 'graph2d' && (
          <div className="btn-group" role="group" aria-label={t('graph.layout')}>
            {LAYOUTS.map((l) => (
              <button key={l} type="button" className={`btn btn-sm ${layout === l ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={layout === l}
                onClick={() => { setLayout(l); store(LAYOUT_KEY, l); }}>{t(`graph.layout.${l}`)}</button>
            ))}
          </div>
        )}
        {is3d && (
          <button type="button" className={`btn btn-sm ${settings.panel ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={settings.panel} aria-expanded={settings.panel}
            data-testid="graph-settings-toggle" onClick={() => change({ panel: !settings.panel })}>⚙ {t('graph.settings')}</button>
        )}
        {is3d && (
          <button type="button" className={`btn btn-sm ${replay.state.playing ? 'btn-primary' : 'btn-secondary'}`} aria-pressed={replay.state.playing} data-testid="graph-replay"
            title={t('graph.replay.hint')} onClick={() => (replay.state.playing ? replay.stop() : void startReplay())}>{replay.state.playing ? '■' : '▶'} {t('graph.replay')}</button>
        )}
        {is3d && (
          <button type="button" className="btn btn-secondary btn-sm" disabled={checking} onClick={() => void checkAll()} title={t('graph.checkHint')}>⟳ {t(checking ? 'graph.checking' : 'graph.check')}</button>
        )}
        {is3d && (
          <button type="button" className="btn btn-secondary btn-sm" onClick={() => setEditing(true)} title={t('topology.note')}>✎ {t('topology.edit')}</button>
        )}
        {is3d && local && (
          <span className="graph-local-chip" data-testid="graph-local-chip">
            ◎ {t('graph.local.chip', { name: localName, depth: local.depth })}
            <button type="button" onClick={() => setLocal(null)} aria-label={t('graph.local.back')} title={t('graph.local.back')}>✕</button>
          </span>
        )}
        <div className="session-graph-legend">
          {!is3d && <span className="session-graph-key is-lead">★ {t('graph.legend.lead')}</span>}
          {!is3d && <span className="session-graph-key is-busy">● {t('graph.legend.busy')}</span>}
          {!is3d && groups.map(([g, color]) => (
            <span key={g} className="session-graph-key"><i style={{ background: color }} />{g === 'local' ? t('graph.legend.local') : g}</span>
          ))}
          <span className="session-graph-count">{t('graph.count', { sessions: sessions.length, links: links.length })}</span>
        </div>
      </div>
      {problem && <div role="alert" className="message" onClick={() => setProblem('')}>{problem}</div>}
      {replayNote && <div role="status" className="message" onClick={() => setReplayNote('')}>{replayNote}</div>}
      <div className={`session-graph-stage ${picked || machine ? 'has-panel' : ''}`}
        onPointerDownCapture={(e) => { if (!(e.target as HTMLElement).closest('.graph-replay-bar, .graph-settings')) stopReplay(); }}
        onWheelCapture={(e) => { if (!(e.target as HTMLElement).closest('.graph-replay-bar, .graph-settings, .session-graph-panel')) stopReplay(); }}>
        {webgl ? (
          <Suspense fallback={<p className="session-graph-wait">{t('graph.loading')}</p>}>
            {mode === 'graph2d'
              ? <Graph2D graph={graph} layout={layout} selected={selected} onSelect={setSelected} onHoverEdge={onHoverEdge} />
              : <Graph3D graph={graph} selected={selected} onSelect={setSelected} onHoverEdge={onHoverEdge} settings={settings} light={light}
                  reducedMotion={reducedMotion} highlight={matches} flyTo={flyTo} events={events} bursts={bursts}
                  fitKey={local ? `${local.id}:${local.depth}` : ''} />}
          </Suspense>
        ) : <p className="session-graph-wait" role="alert">{t('graph.noWebgl')}</p>}
        <p className="session-graph-hint">{t(mode === 'graph2d' ? 'graph.hint2d' : 'graph.hint3d')}</p>
        {is3d && settings.panel && (
          <GraphSettingsPanel settings={settings} onChange={change} search={search} onSearch={setSearch} matches={matches.length}
            onSearchEnter={() => { if (matches[0]) setFlyTo({ id: matches[0], seq: (flyTo?.seq || 0) + 1 }); }}
            groups={groups} devices={topology?.devices.length || 0} tiered={graph.nodes.some((n) => n.kind === 'device')} />
        )}
        {is3d && replay.state.playing && (
          <ReplayBar state={replay.state} speed={settings.replaySpeed} onStop={replay.stop}
            onSpeed={(v) => { change({ replaySpeed: v }); replay.setSpeed(v); }} />
        )}
        {machine && <AttackPanel id={machineId} name={machine.name} sub={machine.sub} onClose={() => setSelected(null)} />}
        {picked && (
          <SessionPanel picked={picked} sessions={sessions} links={links} onAttach={onAttach} onPopOut={onPopOut} onClose={() => setSelected(null)}
            onLocal={is3d ? (depth) => setLocal({ id: picked.id, depth }) : undefined} />
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

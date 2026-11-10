import React, { useState } from 'react';
import { useT } from '../../../shared/i18n';
import ClaudeAddress from '../../terminal/ClaudeAddress';
import type { SessionLink } from '../SessionLinks';
import type { SessionInfo } from '../SessionsView';
import AgentIcons from './AgentIcons';

interface Props {
  picked: SessionInfo;
  sessions: SessionInfo[];
  links: SessionLink[];
  onAttach: (session: SessionInfo) => void;
  onPopOut: (session: SessionInfo) => void;
  onClose: () => void;
  /** The 3D map: show only what is within `depth` links of this session. */
  onLocal?: (depth: number) => void;
}

/** The session picked on the graph: its Claude, who it hands work to and takes it from, its screen, and a way in. */
export default function SessionPanel({ picked, sessions, links, onAttach, onPopOut, onClose, onLocal }: Props) {
  const { t } = useT();
  const [depth, setDepth] = useState(1);
  const incoming = links.filter((l) => l.to === picked.id);
  const outgoing = links.filter((l) => l.from === picked.id);
  const nameOf = (id: string) => { const s = sessions.find((x) => x.id === id); return s?.claude?.[0]?.name || s?.title || s?.label || id; };
  return (
    <aside className="session-graph-panel" aria-label={picked.title || picked.label}>
      <div className="session-graph-panel-head">
        <strong>{picked.lead ? '★ ' : ''}{picked.title || picked.label}</strong>
        <button type="button" className="btn btn-secondary btn-sm" onClick={onClose} aria-label={t('graph.close')}>✕</button>
      </div>
      <code className="session-label">{picked.label}</code>
      <div className="session-graph-badges">
        {picked.lead && <span className="session-work-badge is-orchestrator">{t('claudePeer.lead', { count: outgoing.length })}</span>}
        {picked.busy && <span className="session-work-badge">{t('claudePeer.working')}</span>}
        {!picked.alive && <span className="session-work-badge is-still">{t('graph.ended')}</span>}
      </div>
      <AgentIcons kinds={picked.agents} claude={picked.claude?.[0]} />
      <ClaudeAddress variant="card" peers={picked.claude || []} />
      {onLocal && (
        <div className="session-graph-local">
          <button type="button" className="btn btn-secondary btn-sm" data-testid="graph-local" onClick={() => onLocal(depth)}>◎ {t('graph.local.button')}</button>
          <select className="graph-local-depth" value={depth} aria-label={t('graph.local.depthLabel')} onChange={(e) => setDepth(Number(e.target.value))}>
            {[1, 2].map((d) => <option key={d} value={d}>{t('graph.local.depth', { depth: d })}</option>)}
          </select>
        </div>
      )}
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
  );
}

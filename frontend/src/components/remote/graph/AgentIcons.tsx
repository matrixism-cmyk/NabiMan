import React from 'react';
import './session-graph.css';

/** The AI coding agents shown, each with a mark, a name and a colour of its own. */
export const AGENTS: Record<string, { mark: string; name: string; color: string }> = {
  claude: { mark: '✳', name: 'Claude Code', color: '#f0a67a' },
  codex: { mark: '◎', name: 'Codex', color: '#7dd3fc' },
  gemini: { mark: '✦', name: 'Gemini CLI', color: '#a78bfa' },
  aider: { mark: '▲', name: 'aider', color: '#4ade80' },
  opencode: { mark: '◈', name: 'opencode', color: '#fbbf24' },
  cursor: { mark: '➤', name: 'Cursor Agent', color: '#e2e8f0' },
  copilot: { mark: '⬡', name: 'Copilot CLI', color: '#f472b6' },
};

/** Small marks for the agents running in a session, in a fixed order. */
/** `claude-opus-5-5` as people say it: `opus-5-5`. */
export const shortModel = (m?: string) => (m || '').replace(/^claude-/, '');

export default function AgentIcons({ kinds, claude }: { kinds?: string[]; claude?: { model?: string; version?: string } }) {
  const known = (kinds || []).filter((k) => AGENTS[k]);
  if (!known.length) return null;
  return (
    <span className="agent-icons" aria-label={known.map((k) => AGENTS[k].name).join(', ')}>
      {known.map((k) => (
        <span key={k} className="agent-icon" title={AGENTS[k].name} style={{ color: AGENTS[k].color, borderColor: AGENTS[k].color }}>
          <span aria-hidden="true">{AGENTS[k].mark}</span> {AGENTS[k].name}
          {k === 'claude' && claude?.model && <b className="agent-model">{shortModel(claude.model)}</b>}
          {k === 'claude' && claude?.version && <span className="agent-version">v{claude.version}</span>}
        </span>
      ))}
    </span>
  );
}

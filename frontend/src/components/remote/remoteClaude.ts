import { useEffect, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import type { ClaudePeer } from '../terminal/ClaudeAddress';

/** A Claude session on a server the tiles are connected to, under none of them. */
export interface RemoteClaudeCard {
  /** `rc:<server>:<session>` on a connected server, `lc:<session>` on this machine. */
  id: string;
  /** The saved server's name; empty for this machine. */
  server: string;
  /** A Claude Code background session (run by its daemon, not in a terminal). */
  background?: boolean;
  claude: ClaudePeer[];
  busy: boolean;
  directs: string[];
  orchestrator: boolean;
}

const EVERY_MS = 10000;

/** Remote Claude sessions with no tile, looked up again every few seconds. */
export function useRemoteClaude(enabled: boolean): RemoteClaudeCard[] {
  const [cards, setCards] = useState<RemoteClaudeCard[]>([]);
  useEffect(() => {
    if (!enabled) { setCards([]); return undefined; }
    let live = true;
    const load = () => {
      Promise.resolve(apiRequest<RemoteClaudeCard[]>('/api/terminal/claude-remote'))
        .then((res) => { if (live && res?.success && Array.isArray(res.data)) setCards(res.data); })
        .catch(() => { /* keep what was shown */ });
    };
    load();
    const timer = window.setInterval(load, EVERY_MS);
    return () => { live = false; window.clearInterval(timer); };
  }, [enabled]);
  return cards;
}

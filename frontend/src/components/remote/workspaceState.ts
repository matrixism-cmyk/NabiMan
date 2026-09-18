import { RemoteServer } from '../../types';

/** What the work area is currently about. */
export type Selection =
  | { kind: 'overview' }
  | { kind: 'new' }
  | { kind: 'local' }
  | { kind: 'adhoc' }
  | { kind: 'server'; id: string };

/** Which view of a selected server is showing. */
export type ServerView = 'terminal' | 'status' | 'exec' | 'settings';

/** Which view of the fleet overview is showing. */
export type OverviewView = 'summary' | 'sessions' | 'links' | 'keys';

const SELECTION_KEY = 'nabiman_remote_selection';

export function readSelection(): Selection {
  try {
    const raw = localStorage.getItem(SELECTION_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed.kind === 'string') return parsed as Selection;
    }
  } catch { /* ignore */ }
  return { kind: 'overview' };
}

export function rememberSelection(selection: Selection) {
  try { localStorage.setItem(SELECTION_KEY, JSON.stringify(selection)); } catch { /* ignore */ }
}

/** The tmux label the backend gives a session on this server. */
export function sessionLabel(server: RemoteServer): string {
  return `ssh:${server.user}@${server.host}:${server.port}`;
}

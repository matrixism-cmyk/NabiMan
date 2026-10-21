/** What the work area is currently about. */
export type Selection =
  | { kind: 'overview' }
  | { kind: 'new' }
  | { kind: 'local' }
  | { kind: 'adhoc' }
  | { kind: 'server'; id: string };

/**
 * Which view of a selected server (or the local shell) is showing. A target
 * opens on its sessions, so you see what is already running there before you
 * start anything new.
 */
export type ServerView = 'sessions' | 'terminal' | 'files' | 'status' | 'exec' | 'settings';

/** What the right-click menu on a rail entry can do with it. */
export type RailAction =
  | 'connect' | 'newSession' | 'sessions' | 'window' | 'files'
  | 'status' | 'exec' | 'check' | 'edit' | 'delete';

/** Which view of the fleet overview is showing. */
export type OverviewView = 'summary' | 'sessions' | 'links' | 'keys' | 'settings';

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

/** What the work area had open, so a reload puts the operator back in it. */
export interface PaneState {
  view: ServerView;
  /** Panes whose terminal was opened (by 접속 or a menu), by storage key. */
  connected: string[];
  /** A session joined from a list, if that is what the pane shows. */
  join?: string;
}

const PANE_KEY = 'nabiman_remote_pane';

/**
 * Kept per browser tab: a reload comes back to the terminal that was open,
 * while a fresh visit starts on the session list like a click does.
 */
export function readPaneState(): PaneState {
  try {
    const parsed = JSON.parse(sessionStorage.getItem(PANE_KEY) || 'null');
    if (parsed && typeof parsed.view === 'string' && Array.isArray(parsed.connected)) return parsed as PaneState;
  } catch { /* ignore */ }
  return { view: 'sessions', connected: [] };
}

export function rememberPaneState(state: PaneState) {
  try { sessionStorage.setItem(PANE_KEY, JSON.stringify(state)); } catch { /* ignore */ }
}

/** The tmux label the backend gives a session on this server. */
export { sessionLabel } from './protocols';

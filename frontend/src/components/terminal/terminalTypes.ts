import type { TerminalSettings } from './settings';
import type { ShareConnection } from './terminalUrl';

export interface TerminalTarget {
  kind: 'local' | 'ssh';
  host?: string;
  port?: number;
  user?: string;
  /** Saved remote server id — lets the backend use its stored password. */
  serverId?: string;
  /** Connect as if no password were stored, so it can be typed in the pane. */
  skipSavedPassword?: boolean;
}

export type TerminalStatus = 'connecting' | 'connected' | 'reconnecting' | 'disconnected' | 'ended';

export interface TerminalViewHandle {
  focus: () => void;
  fit: () => void;
  redraw: () => void;
  clear: () => void;
  reconnect: () => void;
  /** Abandon the current tmux session and start a brand new one. */
  newSession: () => void;
  /** Drop a hand-made zoom and go back to filling the window. */
  resetZoom: () => void;
  sessionId: () => string;
  /** Types text at the prompt, as a paste would. */
  insertText: (text: string) => void;
}

export interface TerminalViewProps {
  target: TerminalTarget;
  /** Stable identity of this pane: keys the stored session id and screen buffer. */
  storageKey: string;
  settings: TerminalSettings;
  /** Attach to this existing session instead of the stored/new one. */
  joinSessionId?: string;
  active?: boolean;
  /** Opened through a share link: no account, no session bookkeeping. */
  share?: ShareConnection;
  /** Mirror this grid instead of fitting the window (share viewers). */
  grid?: { cols: number; rows: number };
  onStatus?: (status: TerminalStatus) => void;
  onSessionId?: (id: string) => void;
}




import { TerminalSettings } from './settings';
import { TerminalTarget } from './TerminalView';

interface UrlOptions {
  settings: TerminalSettings;
  /** Set when the pane is opened through a share link instead of an account. */
  share?: { token: string; ticket: string };
  target: TerminalTarget;
  /** Session to attach to; empty starts a new one. */
  sessionId: string;
  /** True when this pane was told to attach to that exact session. */
  join: boolean;
  cols: number;
  rows: number;
}

/** The WebSocket address for one pane, including how it wants to connect. */
export function buildTerminalUrl({ settings, target, sessionId, join, cols, rows, share }: UrlOptions): string {
  const token = sessionStorage.getItem('nabiman_token') || '';
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  if (share) {
    // A visitor has no account: the link (and its ticket) is the credential.
    let url = `${protocol}//${window.location.host}/api/share/${encodeURIComponent(share.token)}/ws`;
    url += `?ticket=${encodeURIComponent(share.ticket)}`;
    if (cols > 0 && rows > 0) url += `&cols=${cols}&rows=${rows}`;
    return url;
  }
  let url = `${protocol}//${window.location.host}/api/terminal?token=${encodeURIComponent(token)}`;
  url += `&scrollback=${settings.scrollback_lines}`;
  // Create/attach at the size this pane already has: a mismatch would make tmux
  // reflow the pane on attach and scroll the first lines out of view.
  if (cols > 0 && rows > 0) url += `&cols=${cols}&rows=${rows}`;
  url += `&keepalive=${settings.keepalive ? 1 : 0}`;
  if (!settings.keepalive) url += `&timeout=${settings.idle_timeout_secs}`;
  if (sessionId) url += `&session_id=${encodeURIComponent(sessionId)}`;
  // An explicit join must attach to *that* session or report it gone; only a
  // remembered id may quietly fall back to starting a fresh session.
  if (sessionId && join) url += '&join=1';
  if (target.kind === 'ssh') {
    if (target.serverId) {
      url += `&server_id=${encodeURIComponent(target.serverId)}`;
      if (target.skipSavedPassword) url += '&no_saved_password=1';
    } else if (target.host) {
      url += `&ssh_host=${encodeURIComponent(target.host)}`;
      url += `&ssh_port=${encodeURIComponent(String(target.port || 22))}`;
      url += `&ssh_user=${encodeURIComponent(target.user || 'root')}`;
    }
  }
  return url;
}

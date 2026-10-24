import { apiRequest } from '../../shared/api/useApi';
import { TerminalSettings } from './settings';
import { TerminalTarget } from './TerminalView';

export interface ShareConnection {
  token: string;
  acquireTicket: (query: string, signal?: AbortSignal) => Promise<string>;
}

interface UrlOptions {
  settings: TerminalSettings;
  /** Set when the pane is opened through a share link instead of an account. */
  share?: Pick<ShareConnection, 'token'>;
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
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  if (share) {
    // The server mirrors the owner's grid; obtain admission separately on each connect.
    return `${protocol}//${window.location.host}/api/share/${encodeURIComponent(share.token)}/ws`;
  }
  let url = `${protocol}//${window.location.host}/api/terminal?scrollback=${settings.scrollback_lines}`;
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
    } else if (target.telnet && target.host) {
      url += `&telnet_host=${encodeURIComponent(target.host)}`;
      url += `&telnet_port=${encodeURIComponent(String(target.port || 23))}`;
      if (target.user) url += `&telnet_user=${encodeURIComponent(target.user)}`;
    } else if (target.host) {
      url += `&ssh_host=${encodeURIComponent(target.host)}`;
      url += `&ssh_port=${encodeURIComponent(String(target.port || 22))}`;
      url += `&ssh_user=${encodeURIComponent(target.user || 'root')}`;
    }
  }
  return url;
}

export async function ticketedShareUrl(url: string, share: ShareConnection, signal?: AbortSignal): Promise<string> {
  const parsed = new URL(url);
  if (!['ws:', 'wss:'].includes(parsed.protocol) || parsed.host !== window.location.host
      || parsed.pathname !== `/api/share/${encodeURIComponent(share.token)}/ws` || parsed.username || parsed.password || parsed.hash
      || parsed.searchParams.has('ticket') || parsed.searchParams.has('token')) throw new Error('Invalid shared terminal address');
  const ticket = await share.acquireTicket(parsed.search.slice(1), signal);
  if (!/^[a-f0-9]{64}$/.test(ticket)) throw new Error('Invalid shared terminal ticket');
  return `${url}${parsed.search ? '&' : '?'}ticket=${ticket}`;
}

/** Each account reconnect obtains its own one-use admission; never put JWTs in URLs. */
export async function ticketedTerminalUrl(url: string, signal?: AbortSignal): Promise<string> {
  const parsed = new URL(url);
  if (!['ws:', 'wss:'].includes(parsed.protocol) || parsed.host !== window.location.host || parsed.pathname !== '/api/terminal' || parsed.hash || parsed.username || parsed.password || parsed.searchParams.has('token') || parsed.searchParams.has('ticket')) {
    throw new Error('Invalid terminal connection address');
  }
  const response = await apiRequest<{ ticket: string; expires_in_seconds: number }>('/api/terminal/tickets', {
    method: 'POST', body: JSON.stringify({ query: parsed.search.slice(1) }), signal,
  });
  if (!response.success) throw new Error(response.message || 'Terminal connection denied');
  const data = response.data;
  if (!data || !/^[0-9a-f]{64}$/.test(data.ticket) || data.expires_in_seconds <= 0 || data.expires_in_seconds > 30) throw new Error('Invalid terminal connection ticket');
  // Preserve the exact original query bytes: URLSearchParams would re-encode them.
  return `${url}${parsed.search ? '&' : '?'}ticket=${data.ticket}`;
}

import { apiRequest, apiPost, fetchWithRefresh } from '../../shared/api/useApi';

export interface VmConsole {
  /** For a built-in bridge this is its VM name; for a registered one, its id. */
  name: string;
  label: string;
  builtin: boolean;
  protocol?: 'vnc' | 'rdp';
  host?: string;
  port?: number;
  user?: string;
  has_password?: boolean;
  can_manage?: boolean;
}

/** The internal VM consoles the account may open (admins/operators). */
export const listConsoles = () => apiRequest<VmConsole[]>('/api/remote/consoles');

export const createConsole = (body: { label: string; host: string; port: number; protocol: string; user?: string; password?: string }) =>
  apiPost<VmConsole>('/api/remote/consoles', body);

export const updateConsole = (id: string, body: { label: string; host: string; port: number; protocol: string; user?: string; password?: string | null }) =>
  apiRequest<VmConsole>(`/api/remote/consoles/${encodeURIComponent(id)}`, { method: 'PUT', body: JSON.stringify(body) });

export const deleteConsole = (id: string) =>
  apiRequest(`/api/remote/consoles/${encodeURIComponent(id)}`, { method: 'DELETE' });

/** A one-use ticket bound to an RDP target (the viewer's display size is not
 *  part of the ticket, so guacamole-common-js may append it freely). */
export async function rdpTicket(target: string, signal?: AbortSignal): Promise<string> {
  const response = await apiRequest<{ ticket: string; expires_in_seconds: number }>('/api/terminal/tickets', {
    method: 'POST', body: JSON.stringify({ query: `target=${target}` }), signal,
  });
  if (!response.success || !response.data) throw new Error(response.message || 'Console connection denied');
  const { ticket, expires_in_seconds: ttl } = response.data;
  if (!/^[0-9a-f]{64}$/.test(ticket) || ttl <= 0 || ttl > 30) throw new Error('Invalid console ticket');
  return ticket;
}

/** A one-use ticketed VNC WebSocket URL, mirroring how a terminal is admitted:
 *  POST the query to get a short-lived ticket, then append it. */
export async function consoleWsUrl(vm: string, signal?: AbortSignal): Promise<string> {
  const query = `vm=${encodeURIComponent(vm)}`;
  const response = await apiRequest<{ ticket: string; expires_in_seconds: number }>('/api/terminal/tickets', {
    method: 'POST', body: JSON.stringify({ query }), signal,
  });
  if (!response.success || !response.data) throw new Error(response.message || 'Console connection denied');
  const { ticket, expires_in_seconds: ttl } = response.data;
  if (!/^[0-9a-f]{64}$/.test(ticket) || ttl <= 0 || ttl > 30) throw new Error('Invalid console ticket');
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/api/remote/console?${query}&ticket=${ticket}`;
}

export interface ConsoleShare { token: string; mode: 'view' | 'control'; expires_unix: number; needs_password?: boolean }

export const createShare = (consoleKey: string, mode: 'view' | 'control', hours: number, password?: string) =>
  apiPost<ConsoleShare>(`/api/remote/consoles/${encodeURIComponent(consoleKey)}/shares`, { mode, hours, password });

export const listShares = (consoleKey: string) =>
  apiRequest<ConsoleShare[]>(`/api/remote/consoles/${encodeURIComponent(consoleKey)}/shares`);

export const revokeShare = (consoleKey: string, token: string) =>
  apiRequest(`/api/remote/consoles/${encodeURIComponent(consoleKey)}/shares/${encodeURIComponent(token)}`, { method: 'DELETE' });

/** The public (no-account) WebSocket URL a share link opens. */
export function shareWsUrl(token: string, pw?: string): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const q = pw ? `?pw=${encodeURIComponent(pw)}` : '';
  return `${protocol}//${window.location.host}/api/cshare/${encodeURIComponent(token)}/ws${q}`;
}

export interface ShareInfo { label: string; protocol: 'vnc' | 'rdp'; mode: 'view' | 'control'; needs_password?: boolean }
export const shareInfo = (token: string) => apiRequest<ShareInfo>(`/api/cshare/${encodeURIComponent(token)}`);

/** The full share link to hand out. */
export const shareLink = (token: string) => `${window.location.origin}/cshare/${token}`;

/** A file on a VM console's redirected "NabiMan" drive. */
export interface DriveFile { name: string; size: number; modified: number }

/** List the files on a VM console's drive (uploads + what the VM copied there). */
export const listDriveFiles = (target: string) =>
  apiRequest<DriveFile[]>(`/api/remote/consoles/${encodeURIComponent(target)}/drive`);

/** Fetch one drive file for download (authenticated). */
export const fetchDriveFile = (target: string, name: string) =>
  fetchWithRefresh(`/api/remote/consoles/${encodeURIComponent(target)}/drive/${encodeURIComponent(name)}`);

/** Stash alternate credentials for one console connect (multi-user VM); returns
 *  a one-use token the WebSocket redeems. Password is never stored. */
export async function consoleCreds(target: string, user: string, password: string): Promise<string> {
  const r = await apiPost<{ token: string }>('/api/remote/console-creds', { target, user, password });
  if (r.success && r.data) return r.data.token;
  throw new Error(r.message || 'credentials rejected');
}

/** A one-use ticket for the serial bridge WebSocket (admin). */
export async function serialTicket(name: string): Promise<string> {
  const response = await apiRequest<{ ticket: string; expires_in_seconds: number }>('/api/terminal/tickets', {
    method: 'POST', body: JSON.stringify({ query: `serial=${name}` }),
  });
  if (!response.success || !response.data) throw new Error(response.message || 'serial denied');
  const { ticket } = response.data;
  if (!/^[0-9a-f]{64}$/.test(ticket)) throw new Error('invalid serial ticket');
  return ticket;
}

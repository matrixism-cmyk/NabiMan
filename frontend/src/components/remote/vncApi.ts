import { apiRequest } from '../../shared/api/useApi';

export interface VmConsole { name: string; label: string }

/** The internal VM consoles the account may open (admins/operators). */
export const listConsoles = () => apiRequest<VmConsole[]>('/api/remote/consoles');

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

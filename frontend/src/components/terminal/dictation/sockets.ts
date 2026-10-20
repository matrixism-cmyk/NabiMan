import { apiRequest } from '../../../hooks/useApi';

const QUERY = 'purpose=dictation';
const wsBase = () => `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}`;

/** A signed-in account's dictation, admitted by a one-use terminal ticket. */
export async function openAccountDictation(): Promise<WebSocket> {
  const res = await apiRequest<{ ticket: string }>('/api/terminal/tickets', {
    method: 'POST', body: JSON.stringify({ query: QUERY }),
  });
  if (!res.success || !res.data || !/^[0-9a-f]{64}$/.test(res.data.ticket)) {
    throw new Error(res.message || 'Dictation was refused');
  }
  return new WebSocket(`${wsBase()}/api/terminal/dictation?${QUERY}&ticket=${res.data.ticket}`);
}

/** A share link visitor's dictation, admitted as the link admits its terminal. */
export async function openShareDictation(
  token: string, acquireTicket: (query: string) => Promise<string>,
): Promise<WebSocket> {
  const ticket = await acquireTicket(QUERY);
  return new WebSocket(`${wsBase()}/api/share/${encodeURIComponent(token)}/dictation?${QUERY}&ticket=${ticket}`);
}

let available: Promise<boolean> | null = null;

/** Whether dictation is set up on this server; asked once per page. */
export function dictationAvailable(): Promise<boolean> {
  available ??= apiRequest<{ available: boolean }>('/api/terminal/dictation/settings')
    .then((r) => Boolean(r.success && r.data?.available))
    .catch(() => false);
  return available;
}

/** Forget the answer, after an administrator changed the settings. */
export function forgetDictationAvailability() {
  available = null;
}

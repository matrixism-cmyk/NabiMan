import { apiRequest } from '../../../shared/api/useApi';

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

export interface DictationSetup {
  available: boolean;
  /** How long a pause ends what was said; 0: only "엔터" does. */
  endpoint_ms: number;
  /** In a terminal, press Enter at each pause too. */
  auto_enter: boolean;
  /** Take only the voice enrolled with the service's account. */
  voice_lock: boolean;
  /** Read each dictated line's answer aloud ("음성으로 대답하기"). */
  voice_reply: boolean;
}

const NONE: DictationSetup = { available: false, endpoint_ms: 0, auto_enter: false, voice_lock: false, voice_reply: false };
let setup: Promise<DictationSetup> | null = null;

/** How dictation is set up on this server; asked once per page. */
export function dictationSetup(): Promise<DictationSetup> {
  setup ??= apiRequest<Partial<DictationSetup>>('/api/terminal/dictation/settings')
    .then((r) => (r.success && r.data ? {
      available: Boolean(r.data.available), endpoint_ms: r.data.endpoint_ms ?? 1500, auto_enter: Boolean(r.data.auto_enter), voice_lock: Boolean(r.data.voice_lock),
      voice_reply: Boolean(r.data.voice_reply),
    } : NONE))
    .catch(() => NONE);
  return setup;
}

/** Whether dictation is set up on this server. */
export function dictationAvailable(): Promise<boolean> {
  return dictationSetup().then((s) => s.available);
}

/** Forget the answer, after an administrator changed the settings. */
export function forgetDictationAvailability() {
  setup = null;
}

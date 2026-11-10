import { apiRequest } from '../../shared/api/useApi';

/** The checklist's steps, in the order they are shown. */
export const STEPS = ['password', 'server', 'terminal', 'backup', 'users', 'guide'] as const;
export type Step = typeof STEPS[number];

/** What this browser remembers of one account's checklist. */
export interface FirstRunState { dismissed: boolean; done: Step[] }

const key = (user: string) => `nabiman_first_run:${user}`;

export function readFirstRun(user: string): FirstRunState {
  try {
    const parsed = JSON.parse(localStorage.getItem(key(user)) || 'null');
    if (parsed && typeof parsed === 'object') {
      const done = Array.isArray(parsed.done) ? parsed.done.filter((s: unknown): s is Step => STEPS.includes(s as Step)) : [];
      return { dismissed: parsed.dismissed === true, done };
    }
  } catch { /* storage blocked or unreadable: as if never seen */ }
  return { dismissed: false, done: [] };
}

export function writeFirstRun(user: string, state: FirstRunState) {
  try { localStorage.setItem(key(user), JSON.stringify(state)); } catch { /* storage blocked: it shows again next visit */ }
}

/**
 * Whether the install looks new: no saved server and no terminal session. An
 * answer that cannot be read counts as "not new", so a hiccup never pops the
 * checklist up on a busy install.
 */
export async function looksFresh(): Promise<boolean> {
  try {
    const [servers, sessions] = await Promise.all([
      apiRequest<unknown[]>('/api/remote-servers'),
      apiRequest<unknown[]>('/api/terminal/sessions'),
    ]);
    return servers.success && sessions.success
      && Array.isArray(servers.data) && servers.data.length === 0
      && Array.isArray(sessions.data) && sessions.data.length === 0;
  } catch {
    return false;
  }
}

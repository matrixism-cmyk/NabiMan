import { useEffect, useState } from 'react';
import { apiRequest } from '../../shared/api/useApi';
import { useMember } from '../remote/member';

/** What `GET /api/orchestration/guide` answers. */
export interface OrchestrationGuide {
  /** The guide (Korean markdown), headed by who this session is. */
  text: string;
  lead: { address: string; tile: string } | null;
  updated: string | null;
}

/** How long a fetched guide is reused for the same session. */
export const GUIDE_TTL_MS = 60_000;

const guides = new Map<string, { at: number; guide: Promise<OrchestrationGuide> }>();
let role: Promise<string> | null = null;

/** Signing in or out (as someone else, maybe) forgets what was fetched. */
export function forgetGuides() {
  guides.clear();
  role = null;
}
if (typeof window !== 'undefined') window.addEventListener('nabiman_auth_changed', forgetGuides);

/** The guide for this session, fetched at most once a minute. A failed fetch is not kept. */
export function fetchGuide(sessionId: string): Promise<OrchestrationGuide> {
  const kept = guides.get(sessionId);
  if (kept && Date.now() - kept.at < GUIDE_TTL_MS) return kept.guide;
  const guide = Promise.resolve()
    .then(() => apiRequest<OrchestrationGuide>(`/api/orchestration/guide?session=${encodeURIComponent(sessionId)}`))
    .then((res) => {
      if (!res.success || !res.data) throw new Error(res.message || '');
      return res.data;
    });
  guides.set(sessionId, { at: Date.now(), guide });
  guide.catch(() => { if (guides.get(sessionId)?.guide === guide) guides.delete(sessionId); });
  return guide;
}

/**
 * Whether this account may have the guide: everyone but a remote-only member,
 * whom the server turns away. `null` while that is not known yet. Floating
 * windows sit outside the workspace's member context, so the account itself
 * is asked (once for the whole page).
 */
export function useGuideAllowed(): boolean | null {
  const member = useMember();
  const [allowed, setAllowed] = useState<boolean | null>(member ? false : null);
  useEffect(() => {
    if (member) { setAllowed(false); return; }
    let live = true;
    role ??= Promise.resolve()
      .then(() => apiRequest<{ role: string }>('/api/users/me'))
      .then((res) => {
        if (!res.success || !res.data) throw new Error(res.message || '');
        return res.data.role;
      });
    const asked = role;
    asked.then((r) => { if (live) setAllowed(r !== '' && r !== 'remote'); })
      .catch(() => { if (role === asked) role = null; if (live) setAllowed(false); });
    return () => { live = false; };
  }, [member]);
  return allowed;
}

/**
 * The guide as one bracketed paste: a Claude Code prompt (or any editor that
 * asked for bracketed paste) takes it as a single pasted block, line breaks
 * and all, and nothing is sent until Enter is pressed. Control characters —
 * an end-of-paste marker above all — are taken out so the text cannot end the
 * paste early, and no line break is left at the end.
 */
export function bracketedPaste(text: string): string {
  const clean = text
    .replace(/\r\n?/g, '\n')
    // eslint-disable-next-line no-control-regex
    .replace(/[\x00-\x08\x0b-\x1f\x7f]/g, '')
    .replace(/\s+$/, '');
  return `\x1b[200~${clean}\x1b[201~`;
}

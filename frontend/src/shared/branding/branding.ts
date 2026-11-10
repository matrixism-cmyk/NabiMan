import { useEffect, useState } from 'react';
import { apiRequest } from '../api/useApi';

/** The install's own touches (GET /api/branding); see backend/src/branding.rs. */
export interface FeedbackWidget { script_url: string; origin: string; site: string }
export interface Branding {
  product_name: string;
  /** Off (null) unless the install configures a feedback service. */
  feedback_widget: FeedbackWidget | null;
  /** The NOC's on-call contact, or null. */
  noc_contact: string | null;
}

let cached: Promise<Branding | null> | null = null;

/** The branding, asked once per page; null when it cannot be read (e.g. a member). */
export function loadBranding(): Promise<Branding | null> {
  if (!cached) {
    cached = apiRequest<Branding>('/api/branding').then((r) => (r.success && r.data ? r.data : null)).catch(() => null);
    // A failure is not kept: signing in later may be allowed to read it.
    cached.then((b) => { if (!b) cached = null; });
  }
  return cached;
}

/** For tests and after signing out. */
export function forgetBranding() { cached = null; }

/** The branding once `enabled` (signed in), else null. */
export function useBranding(enabled: boolean): Branding | null {
  const [branding, setBranding] = useState<Branding | null>(null);
  useEffect(() => {
    if (!enabled) { setBranding(null); return undefined; }
    let live = true;
    loadBranding().then((b) => { if (live) setBranding(b); });
    return () => { live = false; };
  }, [enabled]);
  return branding;
}

const MARK = 'data-nabiman-feedback-script';

/**
 * Adds the feedback launcher to the page, once, with its origin and site as
 * data attributes (the launcher reads them). Nothing when not configured.
 * The CSP allows exactly this origin (security_headers.rs).
 */
export function injectFeedbackWidget(widget: FeedbackWidget | null, doc: Document = document): boolean {
  if (!widget || doc.querySelector(`script[${MARK}]`)) return false;
  const script = doc.createElement('script');
  script.src = widget.script_url;
  script.defer = true;
  script.setAttribute(MARK, '');
  script.setAttribute('data-origin', widget.origin);
  script.setAttribute('data-site', widget.site);
  doc.body.appendChild(script);
  return true;
}

import { redact } from '../../shared/ui/crashReport';

/** One optional program, as GET /api/diagnostics/runtime reports it. */
export interface AboutTool { name: string; found: boolean; version: string | null; features: string[] }
/** The parts of GET /api/diagnostics/runtime the About screen shows. */
export interface AboutRuntime {
  package_version?: string;
  git_commit?: string;
  tools?: { checked_at: string; tools: AboutTool[] } | null;
}
/** The parts of GET /api/license the About screen shows. */
export interface AboutLicense {
  active: boolean;
  tier: string;
  is_trial: boolean;
  trial_days_remaining: number | null;
  expires_at: string | null;
  validation_state?: string;
}

/** Found programs with their versions, then the missing ones that switch something off. */
export function toolsSummary(runtime: AboutRuntime | null | undefined): string {
  const tools = runtime?.tools?.tools;
  if (!tools) return 'not checked';
  const found = tools.filter((x) => x.found).map((x) => (x.version && x.version !== x.name ? `${x.name} ${x.version}` : x.name));
  const missing = tools.filter((x) => !x.found && x.features.length).map((x) => `${x.name} (${x.features.join(', ')})`);
  return [`found: ${found.join(', ') || 'none'}`, `missing: ${missing.join(', ') || 'none'}`].join('; ');
}

/** The license in one line, in English so support reads it the same way. */
export function licenseSummary(license: AboutLicense | null | undefined): string {
  if (!license) return 'unknown';
  const parts = [license.tier || 'free', license.active ? 'active' : 'inactive'];
  if (license.is_trial) parts.push(`trial${license.trial_days_remaining != null ? ` ${license.trial_days_remaining}d left` : ''}`);
  if (license.expires_at) parts.push(`expires ${license.expires_at}`);
  if (license.validation_state) parts.push(license.validation_state);
  return parts.join(', ');
}

/** A mailto: or web link for a contact that is one, else null (a phone number, a name). */
export function contactHref(contact: string): string | null {
  const text = contact.trim();
  if (/^https?:\/\/\S+$/i.test(text)) return text;
  if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(text)) return `mailto:${text}`;
  return null;
}

export interface DiagnosticsInput {
  product: string;
  runtime: AboutRuntime | null | undefined;
  license: AboutLicense | null | undefined;
  lang: string;
  theme: string;
}

/**
 * What "copy diagnostics" puts on the clipboard: which build, which browser,
 * which optional programs. No account, no token, no address of a session:
 * the page is given without its hash, and the text goes through redact().
 */
export function buildDiagnostics({ product, runtime, license, lang, theme }: DiagnosticsInput, now = new Date()): string {
  const lines = [
    `${product} diagnostics`,
    `Time: ${now.toISOString()}`,
    `Version: ${runtime?.package_version || 'unknown'}`,
    `Commit: ${runtime?.git_commit || 'unknown'}`,
    `License: ${licenseSummary(license)}`,
    `Browser: ${navigator.userAgent}`,
    `Language: ${lang} (browser ${navigator.language})`,
    `Theme: ${theme}`,
    `Screen: ${window.screen?.width ?? 0}x${window.screen?.height ?? 0}, window ${window.innerWidth}x${window.innerHeight}, dpr ${window.devicePixelRatio || 1}`,
    `Page: ${window.location.origin}${window.location.pathname}`,
    `Tools: ${toolsSummary(runtime)}`,
  ];
  if (runtime?.tools?.checked_at) lines.push(`Tools checked: ${runtime.tools.checked_at}`);
  return redact(lines.join('\n'));
}

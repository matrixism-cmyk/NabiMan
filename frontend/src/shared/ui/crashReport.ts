import { apiRequest } from '../api/useApi';

/**
 * The text "copy error details" puts on the clipboard: what broke, where, and
 * which build was running. Nothing in it may let the reader act as the user,
 * so share links' tokens, bearer tokens and API keys are blanked out.
 */
export interface CrashDetails {
  error: unknown;
  componentStack?: string;
  region?: string;
}

const SECRETS: [RegExp, string][] = [
  [/eyJ[\w-]+\.[\w-]+\.[\w-]+/g, '[token]'],
  [/\bnbm_[A-Za-z0-9_-]+/g, 'nbm_[key]'],
  [/(Bearer\s+)\S+/gi, '$1[token]'],
  [/\/(c?share)\/[^/\s?#]+/g, '/$1/[link]'],
  [/([?&](?:token|ticket|key|code|state)=)[^&\s#]+/gi, '$1[redacted]'],
];

export function redact(text: string): string {
  return SECRETS.reduce((out, [pattern, replacement]) => out.replace(pattern, replacement), text);
}

export function errorText(error: unknown): string {
  if (error instanceof Error) return `${error.name}: ${error.message}`;
  if (typeof error === 'string') return error;
  try { return JSON.stringify(error); } catch { return String(error); }
}

/** A failed `import()`: the page asked for a file a newer deploy replaced. */
export function isChunkLoadError(error: unknown): boolean {
  const text = error instanceof Error ? `${error.name} ${error.message}` : String(error);
  return /ChunkLoadError|Loading (CSS )?chunk|dynamically imported module|Importing a module script failed/i.test(text);
}

interface Runtime { package_version?: string; git_commit?: string }

async function runtimeVersion(): Promise<string> {
  try {
    const res = await Promise.race([
      apiRequest<Runtime>('/api/diagnostics/runtime'),
      new Promise<never>((_, reject) => setTimeout(() => reject(new Error('timeout')), 3000)),
    ]);
    if (res.success && res.data) return [res.data.package_version, res.data.git_commit].filter(Boolean).join(' @ ') || 'unknown';
  } catch { /* not signed in, not allowed, or offline: the report says so */ }
  return 'unknown';
}

export async function buildCrashReport({ error, componentStack, region }: CrashDetails): Promise<string> {
  const stack = error instanceof Error && error.stack ? error.stack : '';
  const lines = [
    'NabiMan error report',
    `Time: ${new Date().toISOString()}`,
    `Region: ${region || 'app'}`,
    `Page: ${window.location.pathname}${window.location.hash}`,
    `Version: ${await runtimeVersion()}`,
    `Browser: ${navigator.userAgent}`,
    '',
    `Error: ${errorText(error)}`,
  ];
  if (stack) lines.push('', 'Stack:', stack);
  if (componentStack) lines.push('', `Component stack:${componentStack}`);
  return redact(lines.join('\n'));
}

/** Clipboard first; the old execCommand route for browsers that refuse it. */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) { await navigator.clipboard.writeText(text); return true; }
  } catch { /* permission refused: try the fallback below */ }
  try {
    const area = document.createElement('textarea');
    area.value = text;
    area.setAttribute('readonly', '');
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand('copy');
    area.remove();
    return ok;
  } catch { return false; }
}

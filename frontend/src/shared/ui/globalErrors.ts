import { errorText, redact } from './crashReport';

/**
 * Errors no boundary caught (an event handler, a timer, a forgotten promise)
 * end up as `error` / `unhandledrejection` on the window. They used to vanish
 * into the console; now each one becomes a small notice. A failing poll would
 * raise the same error every few seconds, so a message already showing only
 * counts up, one dismissed stays quiet for a while, and a burst is capped.
 */
export interface GlobalNotice { id: number; message: string; count: number }

const QUIET_MS = 60_000;
const BURST_WINDOW_MS = 60_000;
const BURST_MAX = 5;
const VISIBLE_MAX = 3;

let notices: GlobalNotice[] = [];
let nextId = 1;
const lastShown = new Map<string, number>();
let burst: number[] = [];
const listeners = new Set<() => void>();

const emit = () => listeners.forEach((listener) => listener());

export function subscribeNotices(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export const currentNotices = () => notices;

export function dismissNotice(id: number) {
  notices = notices.filter((notice) => notice.id !== id);
  emit();
}

/** Not worth a notice: cancelled requests, the browser's own noise. */
function ignorable(reason: unknown, message: string): boolean {
  if (reason && typeof reason === 'object' && (reason as { name?: string }).name === 'AbortError') return true;
  return /ResizeObserver loop|^Script error\.?$|AbortError/.test(message);
}

export function reportGlobalError(reason: unknown, now = Date.now()) {
  const message = redact(errorText(reason)).slice(0, 300);
  if (!message || ignorable(reason, message)) return;
  const showing = notices.find((notice) => notice.message === message);
  if (showing) {
    notices = notices.map((notice) => (notice === showing ? { ...notice, count: notice.count + 1 } : notice));
    emit();
    return;
  }
  const last = lastShown.get(message);
  if (last !== undefined && now - last < QUIET_MS) return;
  burst = burst.filter((at) => now - at < BURST_WINDOW_MS);
  if (burst.length >= BURST_MAX) return;
  burst.push(now);
  lastShown.set(message, now);
  notices = [...notices, { id: nextId++, message, count: 1 }].slice(-VISIBLE_MAX);
  emit();
}

let installed: (() => void) | null = null;

export function installGlobalErrorHandlers(): () => void {
  if (installed) return installed;
  const onError = (event: ErrorEvent) => reportGlobalError(event.error ?? event.message);
  const onRejection = (event: PromiseRejectionEvent) => reportGlobalError(event.reason);
  window.addEventListener('error', onError);
  window.addEventListener('unhandledrejection', onRejection);
  installed = () => {
    window.removeEventListener('error', onError);
    window.removeEventListener('unhandledrejection', onRejection);
    installed = null;
  };
  return installed;
}

/** Tests start from a clean slate. */
export function resetNoticesForTest() {
  notices = []; lastShown.clear(); burst = []; emit();
}

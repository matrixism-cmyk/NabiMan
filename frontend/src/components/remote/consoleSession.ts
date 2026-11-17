import { ConsoleFault, describeFault, reconnectDelay, RECONNECT_MAX } from './consoleFault';
import type { KeptSession } from './consoleKeeper';

export type ConsoleStatus = 'connecting' | 'connected' | 'disconnected';

/** What the console's bar says under it: the n-th automatic retry, or why it ended. */
export type ConsoleNotice = null | { kind: 'reconnecting'; n: number } | { kind: 'fault'; fault?: ConsoleFault };

/** One try at connecting: `live` turns false once it is replaced or hung up. */
export interface Attempt { live: () => boolean; fail: (fault?: ConsoleFault) => void }

const sameWords = (key: string) => key;

/**
 * A VM console connection that outlives the view showing it: its state, and
 * the retries after an unexpected drop. RDP and VNC add the connection itself.
 * A view attaches while it is on screen and reads `status`/`notice` (told of
 * changes through `watch`); the console keeper holds it in between.
 */
export abstract class ConsoleSession implements KeptSession {
  status: ConsoleStatus = 'connecting';
  notice: ConsoleNotice = null;
  /** The view's "reconnect automatically" setting. */
  autoReconnect = true;
  /** Hung up for good by `close` (until someone asks to reconnect). */
  closed = false;
  /** Whether a view shows this console right now. */
  attached = false;
  private started = false;
  private retries = 0;
  private attemptId = 0;
  private retryTimer?: ReturnType<typeof setTimeout>;
  private watchers = new Set<() => void>();
  private changes = 0;

  /** Starts a connection; the attempt's `fail` reports a drop (once). */
  protected abstract open(attempt: Attempt): void;
  /** Lets go of whatever `open` made. */
  protected abstract shut(): void;

  watch = (changed: () => void) => { this.watchers.add(changed); return () => { this.watchers.delete(changed); }; };
  /** Counts the changes, for useSyncExternalStore. */
  version = () => this.changes;
  alive() { return !this.closed && this.status !== 'disconnected'; }

  protected emit() { this.changes += 1; Array.from(this.watchers).forEach((fn) => fn()); }

  /** Connects when first shown; a session that is already running is left alone. */
  start() {
    if (this.started && !this.closed) return;
    this.started = true;
    this.reconnect();
  }

  /** Connects again from scratch, as the user asked (or a setting changed). */
  reconnect() {
    this.closed = false;
    this.retries = 0;
    this.notice = null;
    this.connect();
  }

  /** Whether `start` has been called: a change of settings then means reconnecting. */
  protected running() { return this.started && !this.closed && this.status !== 'disconnected'; }

  private connect() {
    this.stop();
    const id = this.attemptId;
    let failed = false;
    const live = () => !this.closed && id === this.attemptId;
    this.status = 'connecting';
    this.emit();
    this.open({ live, fail: (fault) => { if (!live() || failed) return; failed = true; this.dropped(fault); } });
  }

  private stop() {
    if (this.retryTimer) clearTimeout(this.retryTimer);
    this.retryTimer = undefined;
    this.attemptId += 1;
    this.shut();
  }

  /** The connection is up. */
  protected up() { this.retries = 0; this.status = 'connected'; this.notice = null; this.emit(); }

  /** The far side closed in an orderly way: nothing to retry. */
  protected ended() { this.stop(); this.status = 'disconnected'; this.emit(); }

  /** An unexpected drop: retry with backoff, unless retrying would only make it
   *  worse (a refused login, a session another login took over). */
  private dropped(fault?: ConsoleFault) {
    const { retry } = describeFault(sameWords, fault);
    if (retry && this.retries < RECONNECT_MAX && this.autoReconnect) {
      const n = ++this.retries;
      this.stop();
      this.status = 'connecting';
      this.notice = { kind: 'reconnecting', n };
      this.retryTimer = setTimeout(() => this.connect(), reconnectDelay(n));
    } else {
      this.stop();
      this.status = 'disconnected';
      this.notice = { kind: 'fault', fault };
    }
    this.emit();
  }

  /** The user read the notice. */
  dismiss() { if (this.notice) { this.notice = null; this.emit(); } }

  /** Hangs up and stays that way. */
  close() {
    this.closed = true;
    this.stop();
    this.status = 'disconnected';
    this.notice = null;
    this.emit();
  }
}

/** The words for a session's notice. */
export function noticeText(t: (key: string, params?: Record<string, string | number>) => string, notice: ConsoleNotice): string {
  if (!notice) return '';
  if (notice.kind === 'reconnecting') return t('remote.console.reconnecting', { n: notice.n, max: RECONNECT_MAX });
  return describeFault(t, notice.fault).message;
}

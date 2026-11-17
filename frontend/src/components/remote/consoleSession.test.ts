import { Attempt, ConsoleSession, noticeText } from './consoleSession';
import { createKeeper } from './consoleKeeper';
import { RECONNECT_MAX, reconnectDelay } from './consoleFault';

/** A session whose "connection" the test drives by hand. */
class TestSession extends ConsoleSession {
  opened: Attempt[] = [];
  shuts = 0;
  protected open(attempt: Attempt) { this.opened.push(attempt); }
  protected shut() { this.shuts += 1; }
  last() { return this.opened[this.opened.length - 1]; }
  connected() { this.up(); }
}

const t = (key: string, params?: Record<string, string | number>) => (params ? `${key} ${JSON.stringify(params)}` : key);

describe('console session', () => {
  beforeEach(() => { jest.useFakeTimers(); });
  afterEach(() => { jest.useRealTimers(); });

  it('connects once, however often a view asks it to start', () => {
    const s = new TestSession();
    s.start(); s.connected(); s.start(); s.start();
    expect(s.opened).toHaveLength(1);
    expect(s.status).toBe('connected');
  });

  it('retries an ordinary drop with backoff, and gives up after the last try', () => {
    const s = new TestSession();
    s.start(); s.connected();
    for (let n = 1; n <= RECONNECT_MAX; n += 1) {
      s.last().fail({ code: 0x0208 });
      expect(s.status).toBe('connecting');
      expect(s.notice).toEqual({ kind: 'reconnecting', n });
      jest.advanceTimersByTime(reconnectDelay(n) - 1);
      expect(s.opened).toHaveLength(n);
      jest.advanceTimersByTime(1);
      expect(s.opened).toHaveLength(n + 1);
    }
    s.last().fail({ code: 0x0208 });
    expect(s.status).toBe('disconnected');
    expect(s.alive()).toBe(false);
    expect(noticeText(t, s.notice)).toBe('remote.console.err.unreachable');
  });

  it('does not reconnect by itself after another login took the session', () => {
    const s = new TestSession();
    s.start(); s.connected();
    s.last().fail({ code: 0x0209, message: 'RDP server closed/refused connection: Disconnected by other connection.' });
    expect(s.status).toBe('disconnected');
    expect(noticeText(t, s.notice)).toBe('remote.console.err.displaced');
    jest.advanceTimersByTime(60_000);
    expect(s.opened).toHaveLength(1);

    // Only when asked.
    s.reconnect();
    expect(s.opened).toHaveLength(2);
    expect(s.status).toBe('connecting');
    expect(s.notice).toBeNull();
  });

  it('does not retry a refused login, or anything when auto-reconnect is off', () => {
    const refused = new TestSession();
    refused.start();
    refused.last().fail({ code: 0x0301 });
    expect(refused.status).toBe('disconnected');

    const manual = new TestSession();
    manual.autoReconnect = false;
    manual.start(); manual.connected();
    manual.last().fail();
    expect(manual.status).toBe('disconnected');
    jest.advanceTimersByTime(60_000);
    expect(manual.opened).toHaveLength(1);
  });

  it('counts one drop once, and ignores a connection that was replaced', () => {
    const s = new TestSession();
    s.start();
    const first = s.last();
    first.fail(); first.fail();
    expect(s.notice).toEqual({ kind: 'reconnecting', n: 1 });
    jest.advanceTimersByTime(reconnectDelay(1));
    expect(first.live()).toBe(false);
    first.fail();
    expect(s.notice).toEqual({ kind: 'reconnecting', n: 1 });
    expect(s.last().live()).toBe(true);
  });

  it('stays down once closed, cancelling a retry that was waiting', () => {
    const s = new TestSession();
    s.start(); s.connected();
    s.last().fail();
    s.close();
    jest.advanceTimersByTime(60_000);
    expect(s.opened).toHaveLength(1);
    expect(s.status).toBe('disconnected');
    expect(s.last().live()).toBe(false);
    // A view that comes up again (or React re-running its effect) starts it afresh.
    s.start();
    expect(s.opened).toHaveLength(2);
  });

  it('is dropped by the keeper when a refused login ends it out of sight', () => {
    const keeper = createKeeper();
    const s = new TestSession();
    keeper.adopt('vm', s);
    s.start(); s.connected();
    keeper.hide('vm');
    expect(keeper.hidden()).toEqual(['vm']);
    // Still retried while hidden: an ordinary drop keeps the console kept.
    s.last().fail();
    expect(keeper.hidden()).toEqual(['vm']);
    jest.advanceTimersByTime(reconnectDelay(1));
    s.last().fail({ code: 0x0301 });
    expect(keeper.get('vm')).toBeUndefined();
    expect(keeper.hidden()).toEqual([]);
  });
});

import { consoleKeeper, createKeeper, KEEP_HIDDEN_MS, KEEP_MAX, KeptSession } from './consoleKeeper';

/** A stand-in connection: up until closed (or until it drops by itself). */
function fakeSession() {
  const watchers = new Set<() => void>();
  const session = {
    up: true,
    close: jest.fn(() => { session.up = false; }),
    alive: () => session.up,
    watch: (fn: () => void) => { watchers.add(fn); return () => { watchers.delete(fn); }; },
    /** The connection ended on its own (a refused login, a dead network). */
    die() { session.up = false; watchers.forEach((fn) => fn()); },
  };
  return session;
}

describe('console keeper', () => {
  beforeEach(() => { jest.useFakeTimers(); jest.setSystemTime(new Date('2026-11-17T09:00:00Z')); });
  afterEach(() => { consoleKeeper.dropAll(); jest.useRealTimers(); });

  it('holds its limits in one place', () => {
    expect(KEEP_HIDDEN_MS).toBe(30 * 60 * 1000);
    expect(KEEP_MAX).toBe(3);
  });

  it('keeps the connection when the view lets go, and hands the same one back', () => {
    const keeper = createKeeper();
    const win = fakeSession();
    keeper.adopt('win2022', win);
    expect(keeper.hidden()).toEqual([]);

    keeper.hide('win2022');
    expect(win.close).not.toHaveBeenCalled();
    expect(keeper.hidden()).toEqual(['win2022']);
    expect(keeper.get('win2022')).toBe(win);

    keeper.show('win2022');
    expect(keeper.hidden()).toEqual([]);
    expect(win.close).not.toHaveBeenCalled();
  });

  it('disconnects a console that stayed out of sight for 30 minutes', () => {
    const keeper = createKeeper();
    const win = fakeSession();
    keeper.adopt('win11', win);
    keeper.hide('win11');

    jest.advanceTimersByTime(KEEP_HIDDEN_MS - 1);
    expect(win.close).not.toHaveBeenCalled();
    jest.advanceTimersByTime(1);
    expect(win.close).toHaveBeenCalledTimes(1);
    expect(keeper.get('win11')).toBeUndefined();
    expect(keeper.hidden()).toEqual([]);
  });

  it('starts the 30 minutes again each time the console is left, and never while it shows', () => {
    const keeper = createKeeper();
    const win = fakeSession();
    keeper.adopt('win11', win);
    keeper.hide('win11');
    jest.advanceTimersByTime(KEEP_HIDDEN_MS - 1000);
    keeper.show('win11');
    jest.advanceTimersByTime(KEEP_HIDDEN_MS * 3);
    expect(win.close).not.toHaveBeenCalled();

    keeper.hide('win11');
    jest.advanceTimersByTime(KEEP_HIDDEN_MS - 1000);
    expect(win.close).not.toHaveBeenCalled();
    jest.advanceTimersByTime(1000);
    expect(win.close).toHaveBeenCalledTimes(1);
  });

  it('holds at most three: a fourth disconnects the one out of sight the longest', () => {
    const keeper = createKeeper();
    const [a, b, c, d] = [fakeSession(), fakeSession(), fakeSession(), fakeSession()];
    keeper.adopt('a', a); keeper.hide('a');
    jest.advanceTimersByTime(60_000);
    keeper.adopt('b', b); keeper.hide('b');
    jest.advanceTimersByTime(60_000);
    keeper.adopt('c', c); keeper.hide('c');
    // Looked at again, then left: `a` is now the most recently hidden.
    jest.advanceTimersByTime(60_000);
    keeper.show('a'); keeper.hide('a');
    expect(keeper.size()).toBe(3);

    keeper.adopt('d', d);
    expect(b.close).toHaveBeenCalledTimes(1);
    expect(a.close).not.toHaveBeenCalled();
    expect(c.close).not.toHaveBeenCalled();
    expect(d.close).not.toHaveBeenCalled();
    expect(keeper.size()).toBe(KEEP_MAX);
    expect(keeper.hidden()).toEqual(['a', 'c']);
  });

  it('disconnects on request without the console being opened', () => {
    const keeper = createKeeper();
    const win = fakeSession();
    keeper.adopt('win11', win); keeper.hide('win11');
    keeper.drop('win11');
    expect(win.close).toHaveBeenCalledTimes(1);
    expect(keeper.hidden()).toEqual([]);
    // Its 30-minute timer went with it.
    jest.advanceTimersByTime(KEEP_HIDDEN_MS * 2);
    expect(win.close).toHaveBeenCalledTimes(1);
  });

  it('keeps nothing for a console whose connection ended while out of sight', () => {
    const keeper = createKeeper();
    const [seen, unseen] = [fakeSession(), fakeSession()];
    keeper.adopt('seen', seen);
    keeper.adopt('unseen', unseen); keeper.hide('unseen');

    unseen.die();
    expect(keeper.get('unseen')).toBeUndefined();
    // On screen, the view shows why and offers to reconnect; the entry goes when it is left.
    seen.die();
    expect(keeper.get('seen')).toBe(seen);
    keeper.hide('seen');
    expect(keeper.get('seen')).toBeUndefined();
    expect(keeper.hidden()).toEqual([]);
  });

  it('drops the consoles the account may no longer open', () => {
    const keeper = createKeeper();
    const [mine, revoked] = [fakeSession(), fakeSession()];
    keeper.adopt('mine', mine); keeper.hide('mine');
    keeper.adopt('revoked', revoked); keeper.hide('revoked');
    keeper.retain(['mine', 'other']);
    expect(revoked.close).toHaveBeenCalledTimes(1);
    expect(mine.close).not.toHaveBeenCalled();
    expect(keeper.hidden()).toEqual(['mine']);
  });

  it('tells its listeners when the kept consoles change, with a stable list in between', () => {
    const keeper = createKeeper();
    const changed = jest.fn();
    const stop = keeper.subscribe(changed);
    keeper.adopt('a', fakeSession());
    expect(changed).not.toHaveBeenCalled();
    keeper.hide('a');
    expect(changed).toHaveBeenCalledTimes(1);
    expect(keeper.hidden()).toBe(keeper.hidden());
    stop();
    keeper.drop('a');
    expect(changed).toHaveBeenCalledTimes(1);
  });

  it('disconnects every kept console when the account signs out', () => {
    const [shown, kept]: (KeptSession & { close: jest.Mock })[] = [fakeSession(), fakeSession()];
    consoleKeeper.adopt('shown', shown);
    consoleKeeper.adopt('kept', kept); consoleKeeper.hide('kept');

    // What clearToken() (sign-out, an expired login) announces.
    window.dispatchEvent(new Event('nabiman_auth_changed'));
    expect(shown.close).toHaveBeenCalledTimes(1);
    expect(kept.close).toHaveBeenCalledTimes(1);
    expect(consoleKeeper.size()).toBe(0);
    expect(consoleKeeper.hidden()).toEqual([]);
  });
});

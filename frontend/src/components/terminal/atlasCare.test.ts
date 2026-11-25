import { careForAtlas, SETTLE_MS } from './atlasCare';

function fakeTerm() {
  let onResize: (() => void) | null = null;
  const term = {
    rows: 24, cleared: 0, refreshed: 0,
    clearTextureAtlas() { term.cleared += 1; },
    refresh() { term.refreshed += 1; },
    onResize(cb: () => void) { onResize = cb; return { dispose: () => { onResize = null; } }; },
    resize() { onResize?.(); },
    listening: () => onResize !== null,
  };
  return term;
}

beforeEach(() => jest.useFakeTimers());
afterEach(() => jest.useRealTimers());

test('a burst of size changes draws the letters afresh once, after it has settled', () => {
  const term = fakeTerm();
  const care = careForAtlas(term as any);
  term.resize(); term.resize(); term.resize();
  jest.advanceTimersByTime(SETTLE_MS - 10);
  expect(term.cleared).toBe(0);
  term.resize();
  jest.advanceTimersByTime(SETTLE_MS + 10);
  expect(term.cleared).toBe(1);
  expect(term.refreshed).toBe(1);
  care.dispose();
});

test('coming back to the tab draws afresh; going away does not', () => {
  const term = fakeTerm();
  const care = careForAtlas(term as any);
  const state = jest.spyOn(document, 'visibilityState', 'get');
  state.mockReturnValue('hidden'); document.dispatchEvent(new Event('visibilitychange'));
  jest.advanceTimersByTime(SETTLE_MS * 2);
  expect(term.cleared).toBe(0);
  state.mockReturnValue('visible'); document.dispatchEvent(new Event('visibilitychange'));
  jest.advanceTimersByTime(SETTLE_MS * 2);
  expect(term.cleared).toBe(1);
  care.dispose(); state.mockRestore();
});

test('disposed, it stops listening and nothing pending runs', () => {
  const term = fakeTerm();
  const care = careForAtlas(term as any);
  term.resize();
  care.dispose();
  jest.advanceTimersByTime(SETTLE_MS * 2);
  expect(term.cleared).toBe(0);
  expect(term.listening()).toBe(false);
  document.dispatchEvent(new Event('visibilitychange'));
  jest.advanceTimersByTime(SETTLE_MS * 2);
  expect(term.cleared).toBe(0);
});

test('a terminal that is already gone does not throw', () => {
  const term = fakeTerm();
  term.clearTextureAtlas = () => { throw new Error('disposed'); };
  careForAtlas(term as any);
  term.resize();
  expect(() => jest.advanceTimersByTime(SETTLE_MS * 2)).not.toThrow();
});

import { careForAtlas, clearLetters, freshLetters, SETTLE_MS } from './atlasCare';

function fakeTerm(log: string[], name: string) {
  let onResize: (() => void) | null = null;
  const term = {
    rows: 24,
    clearTextureAtlas() { log.push(`clear ${name}`); },
    refresh() { log.push(`draw ${name}`); },
    onResize(cb: () => void) { onResize = cb; return { dispose: () => { onResize = null; } }; },
    resize() { onResize?.(); },
  };
  return term;
}

beforeEach(() => jest.useFakeTimers());
afterEach(() => jest.useRealTimers());

test('one terminal changing size redraws every terminal: all forget first, then all draw', () => {
  const log: string[] = [];
  const a = fakeTerm(log, 'a'), b = fakeTerm(log, 'b');
  const careA = careForAtlas(a as any), careB = careForAtlas(b as any);
  jest.advanceTimersByTime(SETTLE_MS + 10); // the newcomers' own redraw
  log.length = 0;
  a.resize(); a.resize(); a.resize();
  jest.advanceTimersByTime(SETTLE_MS - 10);
  expect(log).toEqual([]);
  jest.advanceTimersByTime(20);
  expect(log).toEqual(['clear a', 'clear b', 'draw a', 'draw b']);
  careA.dispose(); careB.dispose();
});

test('a terminal joining or leaving redraws the ones that stay', () => {
  const log: string[] = [];
  const a = fakeTerm(log, 'a');
  const careA = careForAtlas(a as any);
  jest.advanceTimersByTime(SETTLE_MS + 10);
  log.length = 0;
  const b = fakeTerm(log, 'b');
  const careB = careForAtlas(b as any);
  jest.advanceTimersByTime(SETTLE_MS + 10);
  expect(log).toEqual(['clear a', 'clear b', 'draw a', 'draw b']);
  log.length = 0;
  careB.dispose();
  jest.advanceTimersByTime(SETTLE_MS + 10);
  expect(log).toEqual(['clear a', 'draw a']);
  careA.dispose();
  log.length = 0;
  jest.advanceTimersByTime(SETTLE_MS * 3);
  expect(log).toEqual([]);
});

test('clearing one cared-for terminal by hand clears them all at once; a lone terminal only itself', () => {
  const log: string[] = [];
  const a = fakeTerm(log, 'a'), b = fakeTerm(log, 'b'), lone = fakeTerm(log, 'lone');
  const careA = careForAtlas(a as any), careB = careForAtlas(b as any);
  jest.advanceTimersByTime(SETTLE_MS + 10);
  log.length = 0;
  clearLetters(b as any);
  expect(log).toEqual(['clear a', 'clear b', 'draw a', 'draw b']);
  log.length = 0;
  clearLetters(lone as any);
  expect(log).toEqual(['clear lone', 'draw lone']);
  careA.dispose(); careB.dispose();
});

test('coming back to the tab redraws; going away does not', () => {
  const log: string[] = [];
  const a = fakeTerm(log, 'a');
  const care = careForAtlas(a as any);
  jest.advanceTimersByTime(SETTLE_MS + 10);
  log.length = 0;
  const state = jest.spyOn(document, 'visibilityState', 'get');
  state.mockReturnValue('hidden'); document.dispatchEvent(new Event('visibilitychange'));
  jest.advanceTimersByTime(SETTLE_MS * 2);
  expect(log).toEqual([]);
  state.mockReturnValue('visible'); document.dispatchEvent(new Event('visibilitychange'));
  jest.advanceTimersByTime(SETTLE_MS * 2);
  expect(log).toEqual(['clear a', 'draw a']);
  care.dispose(); state.mockRestore();
});

test('a terminal that is already gone is dropped without disturbing the rest', () => {
  const log: string[] = [];
  const a = fakeTerm(log, 'a'), gone = fakeTerm(log, 'gone');
  gone.clearTextureAtlas = () => { throw new Error('disposed'); };
  const careA = careForAtlas(a as any); careForAtlas(gone as any);
  expect(() => freshLetters()).not.toThrow();
  log.length = 0;
  freshLetters();
  expect(log).toEqual(['clear a', 'draw a']);
  careA.dispose();
});

import { toggleAllWindows } from './hideAll';
import type { TermWindow } from './windowGeometry';

const win = (id: string, minimized = false) => ({ id, minimized, x: 1, y: 2, w: 3, h: 4, z: 1 } as TermWindow);
const shown = (windows: TermWindow[]) => windows.filter((w) => !w.minimized).map((w) => w.id);

test('hides every window that shows, and brings exactly those back', () => {
  const start = [win('a'), win('b', true), win('c')];
  const hidden = toggleAllWindows(start, []);
  expect(shown(hidden.windows)).toEqual([]);
  expect(hidden.remembered).toEqual(['a', 'c']);
  // Places and sizes are untouched.
  expect(hidden.windows[0]).toMatchObject({ x: 1, y: 2, w: 3, h: 4 });
  const back = toggleAllWindows(hidden.windows, hidden.remembered);
  expect(shown(back.windows)).toEqual(['a', 'c']);
});

test('with nothing to go by, every window comes back', () => {
  expect(shown(toggleAllWindows([win('a', true), win('b', true)], []).windows)).toEqual(['a', 'b']);
  // The remembered ones were all closed meanwhile.
  expect(shown(toggleAllWindows([win('x', true), win('y', true)], ['a', 'c']).windows)).toEqual(['x', 'y']);
});

test('asked twice with the same input, it answers the same (safe to re-run)', () => {
  const hidden = toggleAllWindows([win('a'), win('b', true)], []);
  const once = toggleAllWindows(hidden.windows, hidden.remembered);
  const twice = toggleAllWindows(hidden.windows, once.remembered);
  expect(shown(twice.windows)).toEqual(shown(once.windows));
});

test('one window opened by hand after hiding: the next press hides it too', () => {
  const hidden = toggleAllWindows([win('a'), win('b')], []);
  const withOne = hidden.windows.map((w) => (w.id === 'b' ? { ...w, minimized: false } : w));
  const again = toggleAllWindows(withOne, hidden.remembered);
  expect(shown(again.windows)).toEqual([]);
  expect(again.remembered).toEqual(['b']);
});

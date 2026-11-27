import { agoText, thumbLines, THUMB_LINES } from './StartMenuParts';

test('a thumbnail is the last lines of the screen, without the blank ones under them', () => {
  const screen = Array.from({ length: 30 }, (_, i) => `line ${i + 1}   `).join('\n') + '\n\n   \n';
  const lines = thumbLines(screen);
  expect(lines.length).toBe(THUMB_LINES);
  expect(lines[0]).toBe('line 23');
  expect(lines[THUMB_LINES - 1]).toBe('line 30');
  expect(thumbLines('$ ls\n\nREADME.md')).toEqual(['$ ls', '', 'README.md']);
});

test('an empty or blank screen has no thumbnail lines', () => {
  expect(thumbLines('')).toEqual([]);
  expect(thumbLines(undefined)).toEqual([]);
  expect(thumbLines(' \n\n  \n')).toEqual([]);
});

test('how long ago a console was last seen', () => {
  const t = (key: string, params?: Record<string, string | number>) => `${key}${params ? `:${params.n}` : ''}`;
  const now = 10 * 3600 * 1000;
  expect(agoText(t, now - 20000, now)).toBe('start.justNow');
  expect(agoText(t, now - 3 * 60000, now)).toBe('start.minutesAgo:3');
  expect(agoText(t, now - 125 * 60000, now)).toBe('start.hoursAgo:2');
});

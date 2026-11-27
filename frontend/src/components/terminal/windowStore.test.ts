import { toggleAllWindows } from './hideAll';
import type { TermWindow } from './windowGeometry';
import {
  cascadeWindows, clampWindows, consoleWindowOf, focusedWindowId, keepListedConsoles, loadLayout, normalizeWindow, placeNew, saveLayout, tileWindows,
} from './windowStore';

const base = { x: 10, y: 20, w: 700, h: 400, z: 11, minimized: false, maximized: false, title: 't', subtitle: 's' };
const term = (id: string, more: Partial<TermWindow> = {}) => ({ ...base, id, kind: 'terminal', target: { kind: 'local' }, ...more } as TermWindow);
const con = (id: string, name: string, more: Partial<TermWindow> = {}) =>
  ({ ...base, id, kind: 'console', title: name, console: { name, label: name, protocol: 'vnc', builtin: true }, ...more } as TermWindow);

beforeEach(() => localStorage.clear());

test('a layout saved before consoles had windows (no kind) loads as terminals', () => {
  localStorage.setItem('nabiman_term_windows:kim', JSON.stringify([
    { ...base, id: 'tw_old', target: { kind: 'ssh', host: 'h', port: 22, user: 'root' } },
    { ...base, id: 'broken' },
    null,
  ]));
  const loaded = loadLayout('nabiman_term_windows:kim');
  expect(loaded).toHaveLength(1);
  expect(loaded[0]).toMatchObject({ id: 'tw_old', kind: 'terminal', x: 10, w: 700, target: { host: 'h' } });
});

test('a console window is saved and loaded with its console; one without a name is dropped', () => {
  saveLayout([term('a'), con('b', 'rocky8')], 'k');
  expect(loadLayout('k').map((w) => w.kind)).toEqual(['terminal', 'console']);
  expect(consoleWindowOf(loadLayout('k'), 'rocky8')?.id).toBe('b');
  expect(normalizeWindow({ ...base, id: 'c', kind: 'console', console: { label: 'x' } })).toBeNull();
  // Whatever else was saved as the protocol, it is one of the two.
  expect(normalizeWindow({ ...base, id: 'c', kind: 'console', console: { name: 'n', protocol: 'spice' } }))
    .toMatchObject({ console: { name: 'n', label: 'n', protocol: 'vnc', builtin: false } });
});

test('tile lays consoles out with terminals and leaves minimised ones alone', () => {
  const tiled = tileWindows([term('a'), con('b', 'vm1'), term('c', { minimized: true }), con('d', 'vm2', { maximized: true })], 1920, 1000);
  const shown = tiled.filter((w) => !w.minimized);
  expect(new Set(shown.map((w) => `${w.x}|${w.y}`)).size).toBe(3);
  expect(shown.every((w) => !w.maximized && w.y + w.h <= 1000 - 44)).toBe(true);
  expect(tiled[2]).toMatchObject({ minimized: true, x: 10, y: 20 });
  expect(tiled.map((w) => w.kind)).toEqual(['terminal', 'console', 'terminal', 'console']);
});

test('cascade steps every showing window, consoles included', () => {
  const stepped = cascadeWindows([con('a', 'vm1', { maximized: true }), term('b'), con('c', 'vm2')], 1920, 1000);
  expect(stepped.map((w) => [w.x, w.y])).toEqual([[40, 60], [72, 92], [104, 124]]);
  expect(stepped[0].maximized).toBe(false);
});

test('hide all / show all counts consoles like terminals', () => {
  const hidden = toggleAllWindows([term('a'), con('b', 'vm1'), con('c', 'vm2', { minimized: true })], []);
  expect(hidden.windows.every((w) => w.minimized)).toBe(true);
  expect(hidden.remembered).toEqual(['a', 'b']);
  const back = toggleAllWindows(hidden.windows, hidden.remembered);
  expect(back.windows.filter((w) => !w.minimized).map((w) => w.id)).toEqual(['a', 'b']);
});

test('the focused window is the top one of those that show', () => {
  expect(focusedWindowId([term('a', { z: 12 }), con('b', 'vm', { z: 15 }), term('c', { z: 20, minimized: true })])).toBe('b');
  expect(focusedWindowId([con('b', 'vm', { minimized: true })])).toBeNull();
});

test('once the console list has loaded, windows of consoles no longer listed go; terminals stay', () => {
  const windows = [term('a'), con('b', 'vm1'), con('c', 'gone')];
  const kept = keepListedConsoles(windows, [{ name: 'vm1', label: 'Rocky 8', protocol: 'rdp', builtin: true }]);
  expect(kept.map((w) => w.id)).toEqual(['a', 'b']);
  // The list's label and protocol win over what was saved.
  expect(kept[1]).toMatchObject({ title: 'Rocky 8', subtitle: 'RDP', console: { label: 'Rocky 8', protocol: 'rdp' } });
  // An account that may not open consoles keeps none.
  expect(keepListedConsoles(windows, null).map((w) => w.id)).toEqual(['a']);
  // Nothing to change: the very same array, so nothing re-renders.
  const same = [term('a'), con('b', 'vm1')];
  expect(keepListedConsoles(same, [{ name: 'vm1', label: 'vm1', builtin: true }])).toBe(same);
});

test('a new window fits the page, and a shrunken page keeps windows within reach', () => {
  expect(placeNew(0, 1920, 1000, { w: 1040, h: 720 })).toEqual({ x: 60, y: 70, w: 1040, h: 720 });
  const small = placeNew(2, 800, 600, { w: 1040, h: 720 });
  expect(small.w).toBe(760);
  expect(small.x + small.w).toBeLessThanOrEqual(800);
  expect(clampWindows([con('a', 'vm', { x: 1800, y: 900 })], 1000, 700)[0]).toMatchObject({ x: 880, y: 620 });
});

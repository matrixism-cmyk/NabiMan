import { SNAP_LAYOUTS, tileZones, unmaximized, zoneAt, zoneRect } from './snapLayouts';
import { TASKBAR_H } from './windowGeometry';

test('every arrangement fills the room exactly, without overlap', () => {
  for (const zones of SNAP_LAYOUTS) {
    const area = zones.reduce((sum, z) => sum + z.w * z.h, 0);
    expect(area).toBeCloseTo(1, 6);
    for (const z of zones) { expect(z.x + z.w).toBeLessThanOrEqual(1 + 1e-9); expect(z.y + z.h).toBeLessThanOrEqual(1 + 1e-9); }
  }
});

test('a zone lands above the terminal bar, a hair inside its share of the page', () => {
  const right = zoneRect(SNAP_LAYOUTS[1][1], 2000, 1044);
  expect(right).toEqual({ x: 1003, y: 3, w: 994, h: 1000 - 6 });
  expect(right.y + right.h).toBeLessThanOrEqual(1044 - TASKBAR_H);
  const quarter = zoneRect(SNAP_LAYOUTS[5][3], 2000, 1044);
  expect(quarter).toEqual({ x: 1003, y: 503, w: 994, h: 494 });
});

test('a tiny page still gives a window that can be used', () => {
  const third = zoneRect(SNAP_LAYOUTS[3][2], 600, 300);
  expect(third.w).toBeGreaterThanOrEqual(320);
  expect(third.h).toBeGreaterThanOrEqual(200);
});

test('the zone under the pointer is read from the picker; anywhere else is none', () => {
  document.body.innerHTML = '<div data-snap="4:2"><span id="in"></span></div><div id="out"></div>';
  const at = (id: string) => ({ elementFromPoint: () => document.getElementById(id) } as unknown as Document);
  expect(zoneAt(1, 1, at('in'))).toBe(SNAP_LAYOUTS[4][2]);
  expect(zoneAt(1, 1, at('out'))).toBeNull();
  document.body.innerHTML = '<div data-snap="9:9" id="bad"></div>';
  expect(zoneAt(1, 1, at('bad'))).toBeNull();
});

test('tiling fills the room whatever the number of windows, a short last row sharing the width', () => {
  for (let n = 1; n <= 12; n += 1) {
    const zones = tileZones(n);
    expect(zones.length).toBe(n);
    expect(zones.reduce((sum, z) => sum + z.w * z.h, 0)).toBeCloseTo(1, 6);
  }
  expect(tileZones(0)).toEqual([]);
  expect(tileZones(3)).toEqual([{ x: 0, y: 0, w: 0.5, h: 0.5 }, { x: 0.5, y: 0, w: 0.5, h: 0.5 }, { x: 0, y: 0.5, w: 1, h: 0.5 }]);
});

test('two tiled windows land exactly where the halves of the picker do', () => {
  const tiled = tileZones(2).map((z) => zoneRect(z, 1920, 1000));
  expect(tiled).toEqual(SNAP_LAYOUTS[1].map((z) => zoneRect(z, 1920, 1000)));
  expect(tileZones(4).map((z) => zoneRect(z, 1920, 1000))).toEqual(SNAP_LAYOUTS[5].map((z) => zoneRect(z, 1920, 1000)));
});

test('a maximised window dragged loose is its old size, under the pointer at the same share of its width', () => {
  expect(unmaximized({ restore: { w: 800, h: 500 } }, 960, 1920)).toEqual({ x: 560, y: 0, w: 800, h: 500 });
  // Grabbed near the right end, it stays inside the page.
  const right = unmaximized({ restore: { w: 800, h: 500 } }, 1900, 1920);
  expect(right.x + right.w).toBeLessThanOrEqual(1920);
  expect(1900 - right.x).toBeGreaterThan(700);
  // Never remembered a size: the usual one.
  expect(unmaximized({}, 0, 1920)).toEqual({ x: 0, y: 0, w: 720, h: 440 });
  // A page narrower than the old size.
  expect(unmaximized({ restore: { w: 1600, h: 900 } }, 300, 800).w).toBeLessThanOrEqual(800);
});

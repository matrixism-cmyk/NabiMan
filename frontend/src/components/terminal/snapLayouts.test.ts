import { SNAP_LAYOUTS, zoneAt, zoneRect } from './snapLayouts';
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

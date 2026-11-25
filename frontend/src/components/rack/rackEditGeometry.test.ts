import { applySteps, checkPlace, clampTop, findDevice, neighbour, netMoves, overlaps, slotFromPointer } from './rackEditGeometry';
import type { Step } from './rackEditGeometry';
import type { Rack, RackDevice, RackLayout } from './rackTypes';

const device = (id: string, u: number | null, h = 1): RackDevice => ({
  id, label: id.toUpperCase(), type: 'server', u, h, u_confirmed: false, node: null, node_candidates: [], ip: null, bmc: null, gpu: null, cpu: null, arch: null, desc: null, check: 'ping', external: false, order_hint: null,
});
const rack = (id: string, devices: RackDevice[], units = 42): Rack => ({ id, name: id, role: null, units, external: false, devices });
const layout: RackLayout = {
  version: 'v1', title: '', sources: [], notes: null, power: [], nodes: [], overlaps: [],
  racks: [rack('R1', [device('sw', 38), device('big', 27, 2), device('loose', null, 2)]), rack('R2', []), rack('R3', [device('t4', 36, 2)], 40)],
};

describe('rack edit geometry', () => {
  it('finds the slot under the pointer, U1 at the bottom, snapped to whole slots', () => {
    // A 42U rack drawn 840 px tall from y=100: 20 px a slot.
    expect(slotFromPointer(100, 100, 840, 42)).toBe(42);
    expect(slotFromPointer(109, 100, 840, 42)).toBe(42);
    expect(slotFromPointer(111, 100, 840, 42)).toBe(41);
    expect(slotFromPointer(100 + 41 * 20, 100, 840, 42)).toBe(1);
    // Picked up 30 px below its top edge: the top edge is what lands.
    expect(slotFromPointer(100 + 5 * 20 + 30, 100, 840, 42, 30)).toBe(37);
    // Outside the drawing it keeps counting; whether that fits is another question.
    expect(slotFromPointer(60, 100, 840, 42)).toBe(44);
    expect(slotFromPointer(100 + 45 * 20, 100, 840, 42)).toBe(-3);
    expect(slotFromPointer(100, 100, 0, 42)).toBeNaN();
  });

  it('keeps a device inside the rack when asked for the nearest place', () => {
    expect(clampTop(44, 2, 42)).toBe(42);
    expect(clampTop(1, 2, 42)).toBe(2);
    expect(clampTop(-3, 1, 42)).toBe(1);
    expect(clampTop(20, 2, 42)).toBe(20);
    expect(clampTop(NaN, 2, 42)).toBe(42);
  });

  it('knows when two spans share a slot', () => {
    expect(overlaps({ top: 27, bottom: 26 }, { top: 26, bottom: 25 })).toBe(true);
    expect(overlaps({ top: 27, bottom: 26 }, { top: 25, bottom: 24 })).toBe(false);
    expect(overlaps({ top: 10, bottom: 1 }, { top: 5, bottom: 5 })).toBe(true);
  });

  it('allows a place that is inside the rack and free of other devices', () => {
    const r1 = layout.racks[0];
    expect(checkPlace(r1, 'sw', 30, 1)).toEqual({ ok: true });
    expect(checkPlace(r1, 'sw', 27, 1)).toEqual({ ok: false, reason: 'overlap', other: 'BIG' });
    expect(checkPlace(r1, 'sw', 26, 1)).toEqual({ ok: false, reason: 'overlap', other: 'BIG' });
    expect(checkPlace(r1, 'big', 28, 2)).toEqual({ ok: true }); // its own old slots do not block it
    expect(checkPlace(r1, 'big', 38, 2)).toEqual({ ok: false, reason: 'overlap', other: 'SW' });
    expect(checkPlace(r1, 'big', 1, 2)).toEqual({ ok: false, reason: 'outside' });
    expect(checkPlace(r1, 'big', 43, 2)).toEqual({ ok: false, reason: 'outside' });
    expect(checkPlace(r1, 'sw', 0, 1)).toEqual({ ok: false, reason: 'outside' });
    expect(checkPlace(r1, 'sw', NaN, 1)).toEqual({ ok: false, reason: 'outside' });
    expect(checkPlace(layout.racks[2], 'sw', 41, 1)).toEqual({ ok: false, reason: 'outside' }); // a 40U rack
    expect(checkPlace(r1, 'loose', 2, 2)).toEqual({ ok: true }); // a device without a slot blocks nothing and may get one
  });

  it('makes the steps in a copy: positions change, a device that changes rack joins the end of the other list', () => {
    const steps: Step[] = [
      { id: 'sw', from: { rack: 'R1', u: 38 }, to: { rack: 'R1', u: 40 } },
      { id: 'big', from: { rack: 'R1', u: 27 }, to: { rack: 'R3', u: 20 } },
      { id: 'sw', from: { rack: 'R1', u: 40 }, to: { rack: 'R3', u: 10 } },
      { id: 'loose', from: { rack: 'R1', u: null }, to: { rack: 'R1', u: 5 } },
    ];
    const moved = applySteps(layout, steps);
    expect(moved.racks[0].devices.map((d) => [d.id, d.u])).toEqual([['loose', 5]]);
    expect(moved.racks[2].devices.map((d) => [d.id, d.u])).toEqual([['t4', 36], ['big', 20], ['sw', 10]]);
    expect(layout.racks[0].devices.map((d) => d.u)).toEqual([38, 27, null]); // the original is untouched
    expect(applySteps(layout, [])).toBe(layout);
    expect(findDevice(moved, 'big')?.rack.id).toBe('R3');
    expect(findDevice(moved, 'nobody')).toBeNull();
    expect(netMoves(layout, steps)).toEqual([{ id: 'big', rack: 'R3', u: 20 }, { id: 'sw', rack: 'R3', u: 10 }, { id: 'loose', rack: 'R1', u: 5 }]);
  });

  it('saves nothing for a device that came back to where it was', () => {
    const steps: Step[] = [
      { id: 'sw', from: { rack: 'R1', u: 38 }, to: { rack: 'R2', u: 10 } },
      { id: 'sw', from: { rack: 'R2', u: 10 }, to: { rack: 'R1', u: 38 } },
    ];
    expect(netMoves(layout, steps)).toEqual([]);
    expect(applySteps(layout, steps).racks[0].devices.map((d) => d.id)).toEqual(['sw', 'big', 'loose']);
  });

  it('names the racks to the left and right', () => {
    expect(neighbour(layout, 'R1', 1)?.id).toBe('R2');
    expect(neighbour(layout, 'R1', -1)).toBeNull();
    expect(neighbour(layout, 'R3', 1)).toBeNull();
    expect(neighbour(layout, 'R3', -1)?.id).toBe('R2');
    expect(neighbour(layout, 'R9', 1)).toBeNull();
  });
});

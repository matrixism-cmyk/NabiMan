import { arrange, deviceBox, fits, slotLabel, slotNumbers, slotSpan } from './rackGeometry';
import type { RackDevice } from './rackTypes';

const device = (id: string, u: number | null, h = 1, order_hint: number | null = null): RackDevice => ({
  id, label: id, type: 'server', u, h, u_confirmed: false, node: null, node_candidates: [], ip: null, bmc: null, gpu: null, cpu: null, arch: null, desc: null, check: 'ping', external: false, order_hint,
});

describe('rack geometry', () => {
  it('takes u as the top slot: a 2U device at u=27 fills U26–U27', () => {
    expect(slotSpan(27, 2)).toEqual({ top: 27, bottom: 26 });
    expect(slotSpan(41, 1)).toEqual({ top: 41, bottom: 41 });
    expect(slotSpan(4, 4)).toEqual({ top: 4, bottom: 1 });
    expect(slotLabel(27, 2)).toBe('U26–27');
    expect(slotLabel(41, 1)).toBe('U41');
  });

  it('puts U1 at the bottom and the highest slot at the top', () => {
    expect(slotNumbers(42)[0]).toBe(42);
    expect(slotNumbers(42)[41]).toBe(1);
    expect(slotNumbers(3)).toEqual([3, 2, 1]);
    // In a 42U rack: slot 42 starts at the top edge, slot 1 ends at the bottom edge.
    expect(deviceBox(42, 1, 42)).toEqual({ top: 0, height: 1 });
    expect(deviceBox(1, 1, 42)).toEqual({ top: 41, height: 1 });
    expect(deviceBox(27, 2, 42)).toEqual({ top: 15, height: 2 });
    expect(deviceBox(2, 2, 42)).toEqual({ top: 40, height: 2 });
    expect(deviceBox(27, 2, 42, 18)).toEqual({ top: 270, height: 36 });
  });

  it('draws neighbours without overlap or gap', () => {
    // U26–27 sits directly on U24–25: one ends where the other begins.
    const upper = deviceBox(27, 2, 42);
    const lower = deviceBox(25, 2, 42);
    expect(upper.top + upper.height).toBe(lower.top);
  });

  it('knows what fits a rack', () => {
    expect(fits(slotSpan(2, 2), 42)).toBe(true);
    expect(fits(slotSpan(42, 1), 42)).toBe(true);
    expect(fits(slotSpan(1, 2), 42)).toBe(false);
    expect(fits(slotSpan(43, 1), 42)).toBe(false);
    expect(fits(slotSpan(0, 1), 42)).toBe(false);
  });

  it('sorts devices into the rack (highest first) and the list under it (by order_hint)', () => {
    const { placed, unplaced } = arrange([
      device('low', 5), device('c', null, 2, 3), device('top', 40), device('nohint', null), device('a', null, 2, 1),
      device('b', null, 2, 2), device('toohigh', 50), device('nohint2', null),
    ], 42);
    expect(placed.map((d) => d.id)).toEqual(['top', 'low']);
    expect(unplaced.map((d) => d.id)).toEqual(['a', 'b', 'c', 'nohint', 'toohigh', 'nohint2']);
  });
});

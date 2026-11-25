import type { RackDevice } from './rackTypes';

/**
 * Where a device sits in a rack. `u` is its TOP slot and U1 is the bottom of
 * the rack: a 2U device at u=27 fills U26–U27.
 */
export interface Span { top: number; bottom: number }

export function slotSpan(u: number, h: number): Span {
  const height = Math.max(1, Math.floor(h) || 1);
  return { top: u, bottom: u - height + 1 };
}

/** Whether the span lies wholly inside a rack of `units` slots. */
export function fits(span: Span, units: number): boolean {
  return span.bottom >= 1 && span.top <= units && span.top >= span.bottom;
}

/**
 * The device's box in a drawing whose slots are `unit` high, measured from
 * the top edge of the rack (where slot `units` is).
 */
export function deviceBox(u: number, h: number, units: number, unit = 1): { top: number; height: number } {
  const span = slotSpan(u, h);
  return { top: (units - span.top) * unit, height: (span.top - span.bottom + 1) * unit };
}

/** Slot numbers as they are written down the side: the highest first. */
export function slotNumbers(units: number): number[] {
  return Array.from({ length: Math.max(0, units) }, (_, i) => units - i);
}

/** "U27" for one slot, "U26–27" for several. */
export function slotLabel(u: number, h: number): string {
  const { top, bottom } = slotSpan(u, h);
  return top === bottom ? `U${top}` : `U${bottom}–${top}`;
}

/**
 * A rack's devices in two piles: those with a slot that fits (drawn in the
 * rack, the highest first) and the rest (listed under it, by `order_hint`,
 * then as the file has them).
 */
export function arrange(devices: RackDevice[], units: number): { placed: RackDevice[]; unplaced: RackDevice[] } {
  const placed: RackDevice[] = [];
  const unplaced: { device: RackDevice; at: number }[] = [];
  devices.forEach((device, at) => {
    if (device.u !== null && fits(slotSpan(device.u, device.h), units)) placed.push(device);
    else unplaced.push({ device, at });
  });
  placed.sort((a, b) => (b.u as number) - (a.u as number));
  unplaced.sort((a, b) => {
    const [ha, hb] = [a.device.order_hint ?? Infinity, b.device.order_hint ?? Infinity];
    return ha === hb ? a.at - b.at : ha - hb;
  });
  return { placed, unplaced: unplaced.map((x) => x.device) };
}

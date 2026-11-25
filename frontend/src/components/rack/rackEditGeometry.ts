import type { Rack, RackDevice, RackLayout } from './rackTypes';
import { Span, fits, slotSpan } from './rackGeometry';

/** Where a device is: its rack and its TOP slot (null: no slot yet). */
export interface Place { rack: string; u: number | null }

/** One move made in the editor; `keys`: made with the arrow keys. */
export interface Step { id: string; from: Place; to: { rack: string; u: number }; keys?: boolean }

export type PlaceCheck = { ok: true } | { ok: false; reason: 'outside' | 'overlap'; other?: string };

/**
 * The top slot a device would get when its top edge is `grab` px above the
 * pointer, in a rack drawn from `rackTop` and `rackHeight` px tall. Snapped
 * to whole slots; not kept inside the rack (see `fits`).
 */
export function slotFromPointer(pointerY: number, rackTop: number, rackHeight: number, units: number, grab = 0): number {
  if (!(rackHeight > 0) || units < 1) return NaN;
  const unit = rackHeight / units;
  return units - Math.round((pointerY - grab - rackTop) / unit);
}

/** The nearest top slot at which a device `h` high lies inside the rack. */
export function clampTop(u: number, h: number, units: number): number {
  return Math.max(Math.min(h, units), Math.min(units, Number.isFinite(u) ? u : units));
}

export function overlaps(a: Span, b: Span): boolean {
  return a.bottom <= b.top && b.bottom <= a.top;
}

/** Whether device `id`, `h` high, may sit with its top at `u` in `rack` as the rack is now. */
export function checkPlace(rack: Rack, id: string, u: number, h: number): PlaceCheck {
  const span = slotSpan(u, h);
  if (!Number.isInteger(u) || !fits(span, rack.units)) return { ok: false, reason: 'outside' };
  const other = rack.devices.find((d) => d.id !== id && d.u !== null && overlaps(span, slotSpan(d.u, d.h)));
  return other ? { ok: false, reason: 'overlap', other: other.label } : { ok: true };
}

export function findDevice(layout: RackLayout, id: string): { rack: Rack; device: RackDevice } | null {
  for (const rack of layout.racks) {
    const device = rack.devices.find((d) => d.id === id);
    if (device) return { rack, device };
  }
  return null;
}

/** Where each moved device ends up, in the order the devices last moved. */
function lastPlaces(steps: Step[]): [string, { rack: string; u: number }][] {
  const last = new Map<string, { rack: string; u: number }>();
  // Re-inserting keeps the order in which devices last moved.
  for (const step of steps) { last.delete(step.id); last.set(step.id, step.to); }
  return Array.from(last.entries());
}

/** The layout with the steps made: a device that changed rack joins the end of its new rack's list. */
export function applySteps(layout: RackLayout, steps: Step[]): RackLayout {
  if (steps.length === 0) return layout;
  const places = lastPlaces(steps);
  const last = new Map(places);
  const arrivals = new Map<string, RackDevice[]>();
  for (const [id, to] of places) {
    const found = findDevice(layout, id);
    if (!found) continue;
    if (found.rack.id !== to.rack) arrivals.set(to.rack, [...(arrivals.get(to.rack) ?? []), { ...found.device, u: to.u }]);
  }
  return {
    ...layout,
    racks: layout.racks.map((rack) => ({
      ...rack,
      devices: [
        ...rack.devices.flatMap((d) => {
          const to = last.get(d.id);
          if (!to) return [d];
          return to.rack === rack.id ? [{ ...d, u: to.u }] : [];
        }),
        ...(arrivals.get(rack.id) ?? []),
      ],
    })),
  };
}

/** What to save: each device that ends somewhere else than the layout has it, once. */
export function netMoves(layout: RackLayout, steps: Step[]): { id: string; rack: string; u: number }[] {
  const moves: { id: string; rack: string; u: number }[] = [];
  for (const [id, to] of lastPlaces(steps)) {
    const found = findDevice(layout, id);
    if (found && (found.rack.id !== to.rack || found.device.u !== to.u)) moves.push({ id, rack: to.rack, u: to.u });
  }
  return moves;
}

/** The racks beside `rack`: `-1` to its left, `1` to its right. */
export function neighbour(layout: RackLayout, rack: string, side: -1 | 1): Rack | null {
  const at = layout.racks.findIndex((r) => r.id === rack);
  return at < 0 ? null : layout.racks[at + side] ?? null;
}

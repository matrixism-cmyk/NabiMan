import { MIN_H, MIN_W, TASKBAR_H } from './windowGeometry';

/** A place a window can snap into: fractions of the room above the terminal bar. */
export interface Zone { x: number; y: number; w: number; h: number }

const H = 1 / 2, T = 1 / 3;

/** The arrangements the picker offers, each a set of zones that together fill the room. */
export const SNAP_LAYOUTS: Zone[][] = [
  [{ x: 0, y: 0, w: 1, h: 1 }],
  [{ x: 0, y: 0, w: H, h: 1 }, { x: H, y: 0, w: H, h: 1 }],
  [{ x: 0, y: 0, w: 2 * T, h: 1 }, { x: 2 * T, y: 0, w: T, h: 1 }],
  [{ x: 0, y: 0, w: T, h: 1 }, { x: T, y: 0, w: T, h: 1 }, { x: 2 * T, y: 0, w: T, h: 1 }],
  [{ x: 0, y: 0, w: H, h: 1 }, { x: H, y: 0, w: H, h: H }, { x: H, y: H, w: H, h: H }],
  [{ x: 0, y: 0, w: H, h: H }, { x: H, y: 0, w: H, h: H }, { x: 0, y: H, w: H, h: H }, { x: H, y: H, w: H, h: H }],
];

/** How close to the top of the page a dragged window brings the picker up. */
export const SNAP_EDGE = 10;
/** While the picker shows, the pointer may be this far down and still keep it. */
export const SNAP_KEEP = 150;

const GAP = 3;

/** The zone in pixels, in a page `vw` by `vh`: inside the room above the bar, a hair apart from its neighbours. */
export function zoneRect(zone: Zone, vw: number, vh: number) {
  const roomH = Math.max(MIN_H, vh - TASKBAR_H);
  const x = Math.round(zone.x * vw) + GAP, y = Math.round(zone.y * roomH) + GAP;
  return {
    x, y,
    w: Math.max(MIN_W, Math.round((zone.x + zone.w) * vw) - GAP - x),
    h: Math.max(MIN_H, Math.round((zone.y + zone.h) * roomH) - GAP - y),
  };
}

/** Which zone of the picker is under a point of the page, if any (the picker marks its zones with data-snap). */
export function zoneAt(x: number, y: number, doc: Document = document): Zone | null {
  const hit = doc.elementFromPoint?.(x, y);
  const mark = hit instanceof Element ? hit.closest('[data-snap]')?.getAttribute('data-snap') : null;
  if (!mark) return null;
  const [layout, index] = mark.split(':').map(Number);
  return SNAP_LAYOUTS[layout]?.[index] || null;
}

/**
 * Places for `count` windows side by side and in rows, filling the room: as
 * square a grid as fits, and a last row with fewer windows shares the whole
 * width among them. The same zones the picker's arrangements are made of, so
 * tiling and dropping onto an arrangement line up to the pixel.
 */
export function tileZones(count: number): Zone[] {
  if (count <= 0) return [];
  const cols = Math.ceil(Math.sqrt(count));
  const rows = Math.ceil(count / cols);
  const zones: Zone[] = [];
  for (let row = 0; row < rows; row += 1) {
    const inRow = row < rows - 1 ? cols : count - cols * (rows - 1);
    for (let col = 0; col < inRow; col += 1) zones.push({ x: col / inRow, y: row / rows, w: 1 / inRow, h: 1 / rows });
  }
  return zones;
}

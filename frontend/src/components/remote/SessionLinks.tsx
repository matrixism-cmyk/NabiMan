import React, { useLayoutEffect, useState } from 'react';

/** One session handing work to another, by session id; `live` while the one it went to works on it. */
export interface SessionLink {
  from: string;
  to: string;
  orchestrator: boolean;
  live: boolean;
  /** How strongly to draw it, 0–1: full while live, fading once the work is done. */
  strength?: number;
  /** Shown on hover: who handed what to whom, and how it is going. */
  note?: string;
}

interface Props {
  /** The grid holding the cards; each card carries `data-session-id`. */
  grid: HTMLDivElement | null;
  links: SessionLink[];
  /** Changes whenever the cards move, so the arrows follow. */
  layout?: string;
}

interface Drawn { key: string; d: string; orchestrator: boolean; live: boolean; strength: number; note?: string }

/** A card's box, relative to the grid. */
export interface Box { l: number; t: number; r: number; b: number }
type Pt = [number, number];

const cx = (k: Box) => (k.l + k.r) / 2;
/** How far an arrow stops short of the card it points at, so its head is seen. */
const STOP = 3;

/** A line through the points with its corners rounded. */
export function rounded(points: Pt[], radius = 8): string {
  const pts = points.filter((p, i) => i === 0 || p[0] !== points[i - 1][0] || p[1] !== points[i - 1][1]);
  const f = (n: number) => n.toFixed(1);
  let d = `M${f(pts[0][0])},${f(pts[0][1])}`;
  for (let i = 1; i < pts.length - 1; i += 1) {
    const [p, q, n] = [pts[i - 1], pts[i], pts[i + 1]];
    const into = Math.hypot(q[0] - p[0], q[1] - p[1]);
    const out = Math.hypot(n[0] - q[0], n[1] - q[1]);
    const r = Math.min(radius, into / 2, out / 2);
    const a: Pt = [q[0] - ((q[0] - p[0]) / into) * r, q[1] - ((q[1] - p[1]) / into) * r];
    const c: Pt = [q[0] + ((n[0] - q[0]) / out) * r, q[1] + ((n[1] - q[1]) / out) * r];
    d += ` L${f(a[0])},${f(a[1])} Q${f(q[0])},${f(q[1])} ${f(c[0])},${f(c[1])}`;
  }
  const last = pts[pts.length - 1];
  return `${d} L${f(last[0])},${f(last[1])}`;
}

/**
 * The way from one card to another through the gaps between cards, never
 * across one: straight to a neighbour, otherwise along the row gap below
 * (or above) the source, down (or up) the column gap beside the target, and
 * in along the row gap next to it. `lane` shifts the line within the gaps so
 * arrows sharing one stay apart; `gx`/`gy` are the grid's column/row gaps.
 */
export function route(a: Box, b: Box, cards: Box[], gx: number, gy: number, lane = 0): Pt[] {
  const rowsOverlap = a.t < b.b && b.t < a.b;
  const colsOverlap = a.l < b.r && b.l < a.r;
  const between = (k: Box) => k !== a && k !== b;
  if (rowsOverlap) {
    const [left, right] = a.l < b.l ? [a, b] : [b, a];
    const blocked = cards.some((k) => between(k) && k.t < Math.min(a.b, b.b) && Math.max(a.t, b.t) < k.b && k.l >= left.r - 1 && k.r <= right.l + 1);
    if (!blocked) {
      const y = (Math.max(a.t, b.t) + Math.min(a.b, b.b)) / 2 + lane;
      const dir = b.l > a.l ? 1 : -1;
      return [[dir > 0 ? a.r : a.l, y], [dir > 0 ? b.l - STOP : b.r + STOP, y]];
    }
    // Others between them in the row: under the row and up into the target.
    const y = Math.max(a.b, b.b) + gy / 2 + lane;
    return [[cx(a) + lane, a.b], [cx(a) + lane, y], [cx(b) + lane, y], [cx(b) + lane, b.b + STOP]];
  }
  const down = b.t >= a.b;
  const leave = down ? a.b : a.t;
  const enter = down ? b.t - STOP : b.b + STOP;
  const gapNearA = down ? a.b + gy / 2 : a.t - gy / 2;
  const gapNearB = down ? b.t - gy / 2 : b.b + gy / 2;
  const blockedColumn = colsOverlap && cards.some((k) => between(k) && k.l < Math.min(a.r, b.r) && Math.max(a.l, b.l) < k.r
    && (down ? k.t >= a.b - 1 && k.b <= b.t + 1 : k.t >= b.b - 1 && k.b <= a.t + 1));
  if (colsOverlap && !blockedColumn) {
    const x = (Math.max(a.l, b.l) + Math.min(a.r, b.r)) / 2 + lane;
    return [[x, leave], [x, enter]];
  }
  const sx = cx(a) + lane;
  const tx = cx(b) + lane;
  // Next rows: across the one gap between them.
  if (Math.abs(gapNearA - gapNearB) < gy) {
    const y = (gapNearA + gapNearB) / 2 + lane;
    return [[sx, leave], [sx, y], [tx, y], [tx, enter]];
  }
  // Rows apart: down the column gap on the source's side of the target
  // (beside the column, for one in the same column).
  let x: number;
  if (colsOverlap) {
    const right = Math.max(...cards.map((k) => k.r));
    x = a.r + gx < right ? a.r + gx / 2 : a.l - gx / 2;
  } else {
    x = cx(b) > cx(a) ? b.l - gx / 2 : b.r + gx / 2;
  }
  x += lane;
  return [[sx, leave], [sx, gapNearA + lane], [x, gapNearA + lane], [x, gapNearB + lane], [tx, gapNearB + lane], [tx, enter]];
}

/** Offsets for the arrows, so two in one gap are not drawn on top of each other: 0, +s, -s, +2s… within the gap. */
const laneOf = (i: number, gap: number) => {
  const step = Math.max(4, Math.round(gap / 5));
  const room = Math.max(0, gap / 2 - 2);
  const n = Math.ceil(i / 2) * step * (i % 2 ? 1 : -1);
  return Math.max(-room, Math.min(room, n));
};

function draw(grid: HTMLDivElement, links: SessionLink[]): Drawn[] {
  const box = grid.getBoundingClientRect();
  // Under a panel zoom (Ctrl + wheel) the page sees the grid scaled; draw in its own units.
  const scale = grid.offsetWidth ? box.width / grid.offsetWidth : 1;
  const style = getComputedStyle(grid);
  const gx = parseFloat(style.columnGap) || 12;
  const gy = parseFloat(style.rowGap) || 12;
  const rel = (r: DOMRect): Box => ({ l: (r.left - box.left) / scale, t: (r.top - box.top) / scale, r: (r.right - box.left) / scale, b: (r.bottom - box.top) / scale });
  const byId = new Map<string, Box>();
  grid.querySelectorAll<HTMLElement>('[data-session-id]').forEach((el) => byId.set(el.dataset.sessionId || '', rel(el.getBoundingClientRect())));
  const cards = Array.from(byId.values());
  const out: Drawn[] = [];
  links.forEach((l, i) => {
    const a = byId.get(l.from);
    const b = byId.get(l.to);
    if (!a || !b) return;
    const d = rounded(route(a, b, cards, gx, gy, laneOf(i, Math.min(gx, gy))));
    out.push({ key: `${l.from}>${l.to}`, d, orchestrator: l.orchestrator, live: l.live, strength: l.strength ?? (l.live ? 1 : 0.2), note: l.note });
  });
  return out;
}

/** Colours for the arrows, one per session handing out work, so each one's arrows read as a set. */
export const LINK_COLORS = ['#ff6b6b', '#4dabf7', '#ffd43b', '#69db7c', '#da77f2', '#ff922b', '#38d9a9', '#f783ac'];
export const colorIndex = (id: string) => {
  let h = 0;
  for (let i = 0; i < id.length; i += 1) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return h % LINK_COLORS.length;
};

interface Hover { key: string; x: number; y: number }

/** What the hover box says: who to whom on top, then the last request and how it is going. */
function LinkTip({ note, x, y, width }: { note: string; x: number; y: number; width: number }) {
  const [head, ...rest] = note.split('\n');
  const left = Math.max(8, Math.min(x + 14, width - 340));
  return (
    <div className="session-link-tip" role="tooltip" style={{ left, top: y + 16 }}>
      <div className="session-link-tip-head">{head}</div>
      {rest.map((line, i) => <div key={i} className="session-link-tip-line">{line}</div>)}
    </div>
  );
}

/**
 * Dashed arrows over the session cards, from a session handing out work to
 * each session it handed work to; the dashes run the way the work goes.
 */
export default function SessionLinks({ grid, links, layout }: Props) {
  const [drawn, setDrawn] = useState<Drawn[]>([]);
  const [hover, setHover] = useState<Hover | null>(null);
  const signature = links.map((l) => `${l.from}>${l.to}:${l.orchestrator}:${l.live}:${(l.strength ?? 0).toFixed(2)}:${l.note ?? ''}`).join(',');

  useLayoutEffect(() => {
    const el = grid;
    if (!el || links.length === 0) { setDrawn([]); return undefined; }
    const redraw = () => setDrawn(draw(el, links));
    redraw();
    const watch = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(redraw) : null;
    watch?.observe(el);
    // A card growing (a longer screen, a new badge) moves the ones after it.
    el.querySelectorAll('[data-session-id]').forEach((c) => watch?.observe(c));
    window.addEventListener('resize', redraw);
    return () => { watch?.disconnect(); window.removeEventListener('resize', redraw); };
    // `signature` stands for `links`, which is rebuilt on every refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [grid, signature, layout]);

  if (drawn.length === 0) return null;
  const at = (e: React.MouseEvent, key: string) => {
    const box = grid?.getBoundingClientRect();
    const scale = box && grid?.offsetWidth ? box.width / grid.offsetWidth : 1;
    if (box) setHover({ key, x: (e.clientX - box.left) / scale, y: (e.clientY - box.top) / scale });
  };
  const hovered = hover ? drawn.find((l) => l.key === hover.key) : undefined;
  const colorOf = (key: string) => colorIndex(key.split('>')[0]);
  return (
    <>
      <svg className={`session-links ${hovered ? 'is-hovering' : ''}`} aria-hidden="true">
        <defs>
          {LINK_COLORS.map((c, i) => (
            <marker key={c} id={`session-link-head-${i}`} viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
              <path d="M0,0 L10,5 L0,10 z" fill={c} />
            </marker>
          ))}
        </defs>
        {drawn.map((l) => {
          const c = colorOf(l.key);
          const on = hovered?.key === l.key;
          return (
            // The line and its head fade together: the opacity is the group's.
            <g key={l.key} className={`${l.live ? 'is-live' : ''} ${on ? 'is-hovered' : ''}`} style={{ opacity: on ? 1 : l.strength }}
              data-link-strength={l.strength.toFixed(2)}>
              {/* A dark band under the line keeps it apart from card borders and other lines. */}
              <path d={l.d} className="session-link-halo" />
              <path d={l.d} data-link={l.key} className={`session-link ${l.orchestrator ? '' : 'is-sub'} ${l.live ? '' : 'is-faint'}`}
                style={{ stroke: LINK_COLORS[c] }} markerEnd={`url(#session-link-head-${c})`} />
              {/* A wide, invisible stroke to hover: the dashed line alone is too thin to find. */}
              {l.note && (
                <path d={l.d} className="session-link-hit" data-link-hit={l.key} aria-label={l.note}
                  onMouseEnter={(e) => at(e, l.key)} onMouseMove={(e) => at(e, l.key)} onMouseLeave={() => setHover(null)} />
              )}
            </g>
          );
        })}
      </svg>
      {hovered?.note && hover && <LinkTip note={hovered.note} x={hover.x} y={hover.y} width={grid?.clientWidth ?? 800} />}
    </>
  );
}

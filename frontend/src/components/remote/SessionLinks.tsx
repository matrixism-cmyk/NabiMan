import React, { useLayoutEffect, useState } from 'react';

/** One session handing work to another, by session id. */
export interface SessionLink { from: string; to: string; orchestrator: boolean }

interface Props {
  /** The grid holding the cards; each card carries `data-session-id`. */
  grid: HTMLDivElement | null;
  links: SessionLink[];
  /** Changes whenever the cards move, so the arrows follow. */
  layout?: string;
}

interface Drawn { key: string; d: string; orchestrator: boolean }

/** Where the line from a box's centre towards a point leaves the box, a little outside it. */
function exit(r: DOMRect, x: number, y: number, gap = 4) {
  const cx = r.left + r.width / 2;
  const cy = r.top + r.height / 2;
  const dx = x - cx;
  const dy = y - cy;
  if (!dx && !dy) return { x: cx, y: cy };
  const t = Math.min(dx ? (r.width / 2 + gap) / Math.abs(dx) : Infinity, dy ? (r.height / 2 + gap) / Math.abs(dy) : Infinity);
  return { x: cx + dx * t, y: cy + dy * t };
}

function draw(grid: HTMLDivElement, links: SessionLink[]): Drawn[] {
  const box = grid.getBoundingClientRect();
  const card = (id: string) => grid.querySelector<HTMLElement>(`[data-session-id="${id.replace(/["\\]/g, '\\$&')}"]`);
  const out: Drawn[] = [];
  for (const l of links) {
    const a = card(l.from)?.getBoundingClientRect();
    const b = card(l.to)?.getBoundingClientRect();
    if (!a || !b) continue;
    const p = exit(a, b.left + b.width / 2, b.top + b.height / 2);
    const q = exit(b, a.left + a.width / 2, a.top + a.height / 2);
    const [x1, y1, x2, y2] = [p.x - box.left, p.y - box.top, q.x - box.left, q.y - box.top];
    // A slight bend, so two arrows between neighbours do not lie on one line.
    const bend = 0.12;
    const mx = (x1 + x2) / 2 - (y2 - y1) * bend;
    const my = (y1 + y2) / 2 + (x2 - x1) * bend;
    out.push({ key: `${l.from}>${l.to}`, d: `M${x1.toFixed(1)},${y1.toFixed(1)} Q${mx.toFixed(1)},${my.toFixed(1)} ${x2.toFixed(1)},${y2.toFixed(1)}`, orchestrator: l.orchestrator });
  }
  return out;
}

/**
 * Dashed arrows over the session cards, from a session handing out work to
 * each session it handed work to; the dashes run the way the work goes.
 */
export default function SessionLinks({ grid, links, layout }: Props) {
  const [drawn, setDrawn] = useState<Drawn[]>([]);
  const signature = links.map((l) => `${l.from}>${l.to}:${l.orchestrator}`).join(',');

  useLayoutEffect(() => {
    const el = grid;
    if (!el || links.length === 0) { setDrawn([]); return undefined; }
    const redraw = () => setDrawn(draw(el, links));
    redraw();
    const watch = typeof ResizeObserver !== 'undefined' ? new ResizeObserver(redraw) : null;
    watch?.observe(el);
    window.addEventListener('resize', redraw);
    return () => { watch?.disconnect(); window.removeEventListener('resize', redraw); };
    // `signature` stands for `links`, which is rebuilt on every refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [grid, signature, layout]);

  if (drawn.length === 0) return null;
  return (
    <svg className="session-links" aria-hidden="true">
      <defs>
        <marker id="session-link-head" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0,0 L10,5 L0,10 z" className="session-link-head" />
        </marker>
        <marker id="session-link-head-sub" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0,0 L10,5 L0,10 z" className="session-link-head is-sub" />
        </marker>
      </defs>
      {drawn.map((l) => (
        <path key={l.key} d={l.d} data-link={l.key} className={`session-link ${l.orchestrator ? '' : 'is-sub'}`}
          markerEnd={`url(#session-link-head${l.orchestrator ? '' : '-sub'})`} />
      ))}
    </svg>
  );
}

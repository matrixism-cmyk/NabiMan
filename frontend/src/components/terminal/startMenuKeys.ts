/** The keys the start menu moves by. */
interface Key { key: string; ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean; preventDefault(): void }

const centre = (el: HTMLElement) => { const r = el.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; };

/** The entry straight above (`-1`) or below (`1`) in the same list: cards sit several to a row. */
function inColumn(items: HTMLElement[], from: HTMLElement, direction: 1 | -1): HTMLElement | undefined {
  const here = centre(from);
  const list = from.closest('section');
  let best: { el: HTMLElement; dy: number; dx: number } | undefined;
  items.forEach((el) => {
    if (el === from || el.closest('section') !== list) return;
    const at = centre(el);
    const dy = (at.y - here.y) * direction;
    if (dy <= 2) return;
    const dx = Math.abs(at.x - here.x);
    if (!best || dy < best.dy - 2 || (Math.abs(dy - best.dy) <= 2 && dx < best.dx)) best = { el, dy, dx };
  });
  return best?.el;
}

/**
 * Arrow keys in the start menu: down from the search box into the entries,
 * then about them as they lie on screen (past the end of one list, on into the
 * next), and up from the first back to the search box. A letter typed on an
 * entry goes to the search box, so searching never needs the mouse.
 */
export function moveFocus(e: Key, menu: HTMLElement) {
  const items = Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"]'));
  const search = menu.querySelector<HTMLInputElement>('input');
  const active = document.activeElement as HTMLElement | null;
  if (active === search) {
    if (e.key === 'ArrowDown' && items[0]) { e.preventDefault(); items[0].focus(); }
    return;
  }
  const at = active ? items.indexOf(active) : -1;
  if (at < 0) return;
  if ((e.key.length === 1 && e.key !== ' ' && !e.ctrlKey && !e.metaKey && !e.altKey) || e.key === 'Backspace') { search?.focus(); return; }
  let next: HTMLElement | null | undefined;
  if (e.key === 'ArrowRight') next = items[at + 1];
  else if (e.key === 'ArrowLeft') next = items[at - 1];
  else if (e.key === 'ArrowDown') next = inColumn(items, items[at], 1) || items[at + 1];
  else if (e.key === 'ArrowUp') next = inColumn(items, items[at], -1) || items[at - 1] || search;
  else if (e.key === 'Home') next = items[0];
  else if (e.key === 'End') next = items[items.length - 1];
  else return;
  e.preventDefault();
  next?.focus();
}

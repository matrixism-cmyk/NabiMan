import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';

export interface RailMenuItem {
  key: string;
  label: string;
  icon?: string;
  danger?: boolean;
  disabled?: boolean;
  /** Draw a divider above this item. */
  separated?: boolean;
  onSelect: () => void;
}

interface Props {
  /** Where the pointer was, in viewport pixels. */
  x: number;
  y: number;
  title: string;
  items: RailMenuItem[];
  onClose: () => void;
  /** The element that opened the menu, focused again once it closes. */
  opener?: HTMLElement | null;
}

/** Room to leave between the menu and the edge of the window. */
const EDGE = 8;

/**
 * The menu a right-click on a rail entry opens.
 *
 * It behaves like a native context menu: it opens where the pointer was (kept
 * inside the window), closes on Escape, a click elsewhere, a scroll or a
 * resize, and is fully usable from the keyboard — arrows to move, Enter to
 * pick, Tab or Escape to leave, with focus going back to the entry it was
 * opened from. Shift+F10 and the menu key open it on a focused entry too,
 * since browsers report those as the same contextmenu event.
 */
export default function RailContextMenu({ x, y, title, items, onClose, opener }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight?: number }>({ left: x, top: y });

  // Keep the whole menu on screen: flip left or up when it would overflow, and
  // scroll inside it when the window is too short for it at all.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const room = window.innerHeight - EDGE * 2;
    const tall = Math.min(height, room);
    const left = x + width + EDGE > window.innerWidth ? Math.max(EDGE, x - width) : x;
    const top = y + tall + EDGE > window.innerHeight ? Math.max(EDGE, window.innerHeight - tall - EDGE) : y;
    setPos({ left, top, maxHeight: height > room ? room : undefined });
  }, [x, y]);

  useEffect(() => {
    const enabled = () => Array.from(
      ref.current?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)') || [],
    );
    enabled()[0]?.focus();

    const close = () => onClose();
    const onPointerDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      const list = enabled();
      const at = list.indexOf(document.activeElement as HTMLButtonElement);
      if (e.key === 'Escape' || e.key === 'Tab') {
        e.preventDefault();
        close();
        opener?.focus();
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const step = e.key === 'ArrowDown' ? 1 : -1;
        list[(at + step + list.length) % list.length]?.focus();
      } else if (e.key === 'Home' || e.key === 'End') {
        e.preventDefault();
        (e.key === 'Home' ? list[0] : list[list.length - 1])?.focus();
      }
    };
    // A scroll anywhere but inside the menu means the entry has moved away.
    const onScroll = (e: Event) => {
      if (ref.current && e.target instanceof Node && ref.current.contains(e.target)) return;
      close();
    };

    document.addEventListener('mousedown', onPointerDown, true);
    document.addEventListener('keydown', onKey, true);
    window.addEventListener('scroll', onScroll, true);
    window.addEventListener('resize', close);
    window.addEventListener('blur', close);
    return () => {
      document.removeEventListener('mousedown', onPointerDown, true);
      document.removeEventListener('keydown', onKey, true);
      window.removeEventListener('scroll', onScroll, true);
      window.removeEventListener('resize', close);
      window.removeEventListener('blur', close);
    };
  }, [onClose, opener]);

  return (
    <div
      ref={ref}
      className="rw-menu"
      role="menu"
      aria-label={title}
      style={{ left: pos.left, top: pos.top, maxHeight: pos.maxHeight, overflowY: pos.maxHeight ? 'auto' : undefined }}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div className="rw-menu-title" aria-hidden="true">{title}</div>
      {items.map(item => (
        <React.Fragment key={item.key}>
          {item.separated && <div className="rw-menu-sep" role="separator" />}
          <button
            type="button"
            role="menuitem"
            className={`rw-menu-item ${item.danger ? 'is-danger' : ''}`}
            disabled={item.disabled}
            onClick={() => { onClose(); item.onSelect(); }}
          >
            <span className="rw-menu-icon" aria-hidden="true">{item.icon || ''}</span>
            {item.label}
          </button>
        </React.Fragment>
      ))}
    </div>
  );
}

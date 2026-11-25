import { useCallback, useEffect, useRef, useState } from 'react';
import type React from 'react';
import type { RackDevice, RackLayout } from './rackTypes';
import { PlaceCheck, checkPlace, clampTop, findDevice, slotFromPointer } from './rackEditGeometry';

/** Where the device being dragged would land: drawn in that rack, in the accent or the danger colour. */
export interface DropTarget { rack: string; u: number; check: PlaceCheck }
export interface Dragging { id: string; label: string; h: number; target: DropTarget | null }

/** The pointer has to travel this far before a press becomes a drag (a tap or a click stays one). */
const SLOP = 5;
/** Slots above or below a rack's drawing that still count as aiming at it. */
const REACH = 3;

interface Held { device: RackDevice; pointer: number; x: number; y: number; grab: number | null; active: boolean; target: DropTarget | null }

/**
 * Dragging with pointer events (mouse, pen and touch alike; no HTML5 drag):
 * a device is picked up by its face and its landing place follows the
 * pointer in whole slots, in whichever rack the pointer is over. Let go on
 * a valid place and `onDrop` is called; anywhere else it stays where it was.
 * Racks are found by their `[data-rack-slots]` element under `root`.
 */
export function useRackDrag(root: React.RefObject<HTMLElement | null>, working: RackLayout, onDrop: (id: string, rack: string, u: number) => void) {
  const [dragging, setDragging] = useState<Dragging | null>(null);
  const held = useRef<Held | null>(null);
  const live = useRef({ working, onDrop });
  live.current = { working, onDrop };

  const aim = useCallback((h: Held, x: number, y: number): DropTarget | null => {
    const layout = live.current.working;
    const height = findDevice(layout, h.device.id)?.device.h ?? h.device.h;
    for (const el of Array.from(root.current?.querySelectorAll<HTMLElement>('[data-rack-slots]') ?? [])) {
      const rack = layout.racks.find((r) => r.id === el.dataset.rackSlots);
      const box = el.getBoundingClientRect();
      if (!rack || !(box.height > 0) || x < box.left || x > box.right) continue;
      const unit = box.height / rack.units;
      if (y < box.top - REACH * unit || y > box.bottom + REACH * unit) continue;
      // A device from the list under a rack is held by its middle.
      const u = slotFromPointer(y, box.top, box.height, rack.units, h.grab ?? (height * unit) / 2);
      const check = checkPlace(rack, h.device.id, u, height);
      return { rack: rack.id, u: !check.ok && check.reason === 'outside' ? clampTop(u, height, rack.units) : u, check };
    }
    return null;
  }, [root]);

  const stop = useCallback(() => { held.current = null; setDragging(null); }, []);

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      const h = held.current;
      if (!h || e.pointerId !== h.pointer) return;
      if (!h.active && Math.hypot(e.clientX - h.x, e.clientY - h.y) < SLOP) return;
      h.active = true;
      h.target = aim(h, e.clientX, e.clientY);
      setDragging({ id: h.device.id, label: h.device.label, h: h.device.h, target: h.target });
    };
    const onUp = (e: PointerEvent) => {
      const h = held.current;
      if (!h || e.pointerId !== h.pointer) return;
      const target = h.active ? aim(h, e.clientX, e.clientY) : null;
      stop();
      if (target?.check.ok) live.current.onDrop(h.device.id, target.rack, target.u);
    };
    const onCancel = (e: PointerEvent) => { if (held.current?.pointer === e.pointerId) stop(); };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape' && held.current?.active) { e.stopPropagation(); stop(); } };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    window.addEventListener('pointercancel', onCancel);
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('pointercancel', onCancel);
      window.removeEventListener('keydown', onKey, true);
    };
  }, [aim, stop]);

  /** For a device's face: start holding it. */
  const onPointerDown = useCallback((e: React.PointerEvent<HTMLElement>, device: RackDevice) => {
    if (e.button !== 0 || e.isPrimary === false || held.current) return;
    const placed = device.u !== null && !e.currentTarget.classList.contains('is-listed');
    held.current = { device, pointer: e.pointerId, x: e.clientX, y: e.clientY, grab: placed ? e.clientY - e.currentTarget.getBoundingClientRect().top : null, active: false, target: null };
  }, []);

  return { dragging, onPointerDown };
}

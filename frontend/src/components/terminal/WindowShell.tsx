import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import SnapPicker from './SnapPicker';
import { LOOSEN_PX, SNAP_EDGE, SNAP_KEEP, unmaximized, Zone, zoneAt, zoneRect } from './snapLayouts';
import { clamp, MIN_H, MIN_W, TASKBAR_H, TermWindow, WindowPatch } from './windowGeometry';

export interface ShellHandlers {
  onUpdate: (id: string, patch: WindowPatch) => void;
  onFocus: (id: string) => void;
}

interface ShellProps extends ShellHandlers {
  win: TermWindow;
  className?: string;
  /** The start of the title bar: what the window is. */
  lead: React.ReactNode;
  /** Title-bar buttons of the window's own, before minimise and maximise. */
  actions?: React.ReactNode;
  /** Title-bar buttons after them: how the window is closed. */
  closing: React.ReactNode;
  /** Dialogs the window opens. */
  extras?: React.ReactNode;
  children: React.ReactNode;
}

/**
 * The frame every floating window shares, whatever it shows: dragged by its
 * title bar, resized by its edges, minimised, maximised (and dragged loose
 * again), dropped onto an arrangement at the top of the page, raised on a
 * press. What is in the bar and in the body is the window's own business.
 */
export default function WindowShell({ win, onUpdate, onFocus, className, lead, actions, closing, extras, children }: ShellProps) {
  const { t } = useT();
  const [snap, setSnap] = useState<{ over: Zone | null } | null>(null);

  const beginDrag = (e: React.PointerEvent) => {
    // Pointer-down on a title-bar button must not start a drag: capturing the
    // pointer on the bar would swallow the pointerup and the button would
    // never see a click.
    // Nor may it while the name is being typed.
    if ((e.target as HTMLElement).closest('button, input, form')) return;
    if (e.button > 0) return; // primary button (or touch) only
    const handle = e.currentTarget as HTMLElement;
    try { handle.setPointerCapture(e.pointerId); } catch { /* unsupported */ }
    const startX = e.clientX;
    const startY = e.clientY;
    let originX = win.x;
    let originY = win.y;
    let width = win.w;
    // A maximised window comes loose once it is really dragged (not on a mere
    // click of its bar): back at the size it had, under the pointer at the
    // same share of its width.
    let height = win.h;
    let held = win.maximized ? unmaximized(win, e.clientX) : null;
    onFocus(win.id);
    // Dragged to the top of the page, the arrangements come up; let go over
    // one of their zones and the window takes that place (snapLayouts).
    let picking = false;
    const move = (ev: PointerEvent) => {
      if (held) {
        if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < LOOSEN_PX) return;
        onUpdate(win.id, { maximized: false, ...held });
        ({ x: originX, y: originY, w: width, h: height } = held);
        held = null;
      }
      onUpdate(win.id, {
        x: clamp(originX + ev.clientX - startX, -width + 120, window.innerWidth - 80),
        y: clamp(originY + ev.clientY - startY, 0, window.innerHeight - 60),
      });
      picking = ev.clientY <= (picking ? SNAP_KEEP : SNAP_EDGE);
      setSnap(picking ? { over: zoneAt(ev.clientX, ev.clientY) } : null);
    };
    const up = (ev: Event) => {
      const zone = picking && ev.type === 'pointerup' ? zoneAt((ev as PointerEvent).clientX, (ev as PointerEvent).clientY) : null;
      if (zone) {
        onUpdate(win.id, {
          ...zoneRect(zone, window.innerWidth, window.innerHeight), maximized: false,
          restore: { x: originX, y: originY, w: width, h: height },
        });
      }
      picking = false;
      setSnap(null);
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      handle.removeEventListener('lostpointercapture', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
    handle.addEventListener('lostpointercapture', up);
  };

  const beginResize = (e: React.PointerEvent, edge: 'se' | 'e' | 's') => {
    e.stopPropagation();
    const handle = e.currentTarget as HTMLElement;
    try { handle.setPointerCapture(e.pointerId); } catch { /* unsupported */ }
    const startX = e.clientX;
    const startY = e.clientY;
    const originW = win.w;
    const originH = win.h;
    onFocus(win.id);
    const move = (ev: PointerEvent) => {
      const patch: WindowPatch = { maximized: false };
      if (edge !== 's') patch.w = Math.max(MIN_W, originW + ev.clientX - startX);
      if (edge !== 'e') patch.h = Math.max(MIN_H, originH + ev.clientY - startY);
      onUpdate(win.id, patch);
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      handle.removeEventListener('lostpointercapture', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
    handle.addEventListener('lostpointercapture', up);
  };

  const toggleMaximize = () => {
    if (win.maximized) {
      const r = win.restore;
      onUpdate(win.id, {
        maximized: false,
        ...(r ? { x: r.x, y: r.y, w: r.w, h: r.h } : {}),
      });
    } else {
      onUpdate(win.id, {
        maximized: true,
        restore: { x: win.x, y: win.y, w: win.w, h: win.h },
      });
    }
  };

  const style: React.CSSProperties = win.maximized
    ? { left: 0, top: 0, width: '100vw', height: `calc(100vh - ${TASKBAR_H}px)`, zIndex: win.z }
    : { left: win.x, top: win.y, width: win.w, height: win.h, zIndex: win.z };

  return (
    <div
      className={`term-window ${win.minimized ? 'term-window-minimized' : ''} ${className || ''}`}
      style={style}
      data-window-kind={win.kind}
      onPointerDown={(e) => { if (!(e.target as HTMLElement).closest('.term-window-actions')) onFocus(win.id); }}
    >
      <div
        className="term-window-bar"
        onPointerDown={beginDrag}
        onDoubleClick={(e) => { if (!(e.target as HTMLElement).closest('button, input, form')) toggleMaximize(); }}
      >
        {lead}
        <span className="term-window-actions">
          {actions}
          <button title={t('terminal.minimize')} onClick={(e) => { e.stopPropagation(); onUpdate(win.id, { minimized: true }); }}>─</button>
          <button title={win.maximized ? t('terminal.restoreSize') : t('terminal.maximize')} onClick={(e) => { e.stopPropagation(); toggleMaximize(); }}>▢</button>
          {closing}
        </span>
      </div>
      <div className="term-window-body">{children}</div>
      {extras}
      {snap && <SnapPicker over={snap.over} />}
      <div className="term-resize term-resize-e" onPointerDown={(e) => beginResize(e, 'e')} />
      <div className="term-resize term-resize-s" onPointerDown={(e) => beginResize(e, 's')} />
      <div className="term-resize term-resize-se" onPointerDown={(e) => beginResize(e, 'se')} />
    </div>
  );
}

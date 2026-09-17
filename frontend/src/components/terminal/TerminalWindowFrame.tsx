import React, { useRef, useState } from 'react';
import TerminalView, { TerminalStatus, TerminalViewHandle } from './TerminalView';
import { useTerminalSettings } from './settings';
import { useT } from '../../i18n';
import { clamp, MIN_H, MIN_W, TASKBAR_H, TermWindow } from './windowGeometry';

interface FrameProps {
  win: TermWindow;
  onUpdate: (id: string, patch: Partial<TermWindow>) => void;
  onFocus: (id: string) => void;
  onClose: (id: string) => void;
  onEndSession: (id: string) => void;
}

export default function TerminalWindowFrame({ win, onUpdate, onFocus, onClose, onEndSession }: FrameProps) {
  const { t } = useT();
  const { settings } = useTerminalSettings();
  const viewRef = useRef<TerminalViewHandle>(null);
  const [status, setStatus] = useState<TerminalStatus>('connecting');

  const beginDrag = (e: React.PointerEvent) => {
    // Pointer-down on a title-bar button must not start a drag: capturing the
    // pointer on the bar would swallow the pointerup and the button would
    // never see a click.
    if ((e.target as HTMLElement).closest('button')) return;
    if (e.button > 0) return; // primary button (or touch) only
    if (win.maximized) return;
    const handle = e.currentTarget as HTMLElement;
    try { handle.setPointerCapture(e.pointerId); } catch { /* unsupported */ }
    const startX = e.clientX;
    const startY = e.clientY;
    const originX = win.x;
    const originY = win.y;
    onFocus(win.id);
    const move = (ev: PointerEvent) => {
      onUpdate(win.id, {
        x: clamp(originX + ev.clientX - startX, -win.w + 120, window.innerWidth - 80),
        y: clamp(originY + ev.clientY - startY, 0, window.innerHeight - 60),
      });
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
      const patch: Partial<TermWindow> = { maximized: false };
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
      className={`term-window ${win.minimized ? 'term-window-minimized' : ''}`}
      style={style}
      onPointerDown={(e) => { if (!(e.target as HTMLElement).closest('.term-window-actions')) onFocus(win.id); }}
    >
      <div
        className="term-window-bar"
        onPointerDown={beginDrag}
        onDoubleClick={(e) => { if (!(e.target as HTMLElement).closest('button')) toggleMaximize(); }}
      >
        <span className={`term-window-dot term-window-dot-${status}`} />
        <span className="term-window-title">{win.title}</span>
        <span className="term-window-sub">{win.subtitle}</span>
        <span className="term-window-actions">
          <button title={t('terminal.reconnect')} onClick={(e) => { e.stopPropagation(); viewRef.current?.reconnect(); }}>↻</button>
          <button title={t('terminal.newSession')} onClick={(e) => { e.stopPropagation(); viewRef.current?.newSession(); }}>+</button>
          <button title={t('terminal.minimize')} onClick={(e) => { e.stopPropagation(); onUpdate(win.id, { minimized: true }); }}>─</button>
          <button title={win.maximized ? t('terminal.restoreSize') : t('terminal.maximize')} onClick={(e) => { e.stopPropagation(); toggleMaximize(); }}>▢</button>
          <button
            title={t('terminal.endSessionHint')}
            onClick={(e) => {
              e.stopPropagation();
              if (window.confirm(t('terminal.endSessionConfirm', { name: win.title }))) {
                onEndSession(win.id);
              }
            }}
          >⏻</button>
          <button
            className="term-window-close"
            title={t('terminal.closeWindowHint')}
            onClick={(e) => { e.stopPropagation(); onClose(win.id); }}
          >✕</button>
        </span>
      </div>
      <div className="term-window-body">
        <TerminalView
          ref={viewRef}
          target={win.target}
          storageKey={win.id}
          settings={settings}
          active={!win.minimized}
          onStatus={setStatus}
        />
      </div>
      <div className="term-resize term-resize-e" onPointerDown={(e) => beginResize(e, 'e')} />
      <div className="term-resize term-resize-s" onPointerDown={(e) => beginResize(e, 's')} />
      <div className="term-resize term-resize-se" onPointerDown={(e) => beginResize(e, 'se')} />
    </div>
  );
}

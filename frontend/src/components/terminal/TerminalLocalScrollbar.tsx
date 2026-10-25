import React, { useEffect, useRef, useState } from 'react';
import type { Terminal } from 'xterm';
import { useT } from '../../shared/i18n';

/**
 * The same ▲▼ scrollbar as the account terminal's history bar, but for a share
 * link's window: a shared viewer must not move the owner's tmux history (every
 * viewer would scroll at once), so this scrolls the viewer's OWN xterm buffer
 * — the scrollback they have received — with click-to-jump, drag and wheel.
 */
export default function TerminalLocalScrollbar({ termRef }: { termRef: React.RefObject<Terminal | null> }) {
  const { t } = useT();
  const controlsRef = useRef<HTMLDivElement>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointer: number; grip: number } | null>(null);
  const [trackH, setTrackH] = useState(0);
  // max = how far back you can scroll; pos = current top line (0 = oldest).
  const [{ max, pos, rows }, setState] = useState({ max: 0, pos: 0, rows: 1 });

  // Mirror the xterm buffer: update on scroll and as output streams in.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return undefined;
    const sync = () => {
      const buf = term.buffer.active;
      setState({ max: buf.baseY, pos: buf.viewportY, rows: term.rows });
    };
    sync();
    const a = term.onScroll(sync);
    const b = term.onRender(sync);
    return () => { a.dispose(); b.dispose(); };
  }, [termRef]);

  useEffect(() => {
    const track = trackRef.current;
    if (!track) return undefined;
    const observer = new ResizeObserver(() => setTrackH(track.clientHeight));
    observer.observe(track);
    return () => observer.disconnect();
  }, []);

  // Wheel anywhere over the bar scrolls the terminal.
  useEffect(() => {
    const el = controlsRef.current;
    if (!el) return undefined;
    const wheel = (e: WheelEvent) => { e.preventDefault(); if (e.deltaY) termRef.current?.scrollLines(Math.sign(e.deltaY) * 3); };
    el.addEventListener('wheel', wheel, { passive: false });
    return () => el.removeEventListener('wheel', wheel);
  }, [termRef]);

  const thumbHeight = max ? Math.min(trackH, Math.max(28, trackH * rows / (rows + max))) : trackH;
  const travel = Math.max(0, trackH - thumbHeight);
  const top = max ? travel * (pos / max) : 0;
  const disabled = max === 0;

  const toLine = (clientY: number, grip: number) => {
    const track = trackRef.current;
    if (!track || !travel) return;
    const fraction = Math.max(0, Math.min(1, (clientY - track.getBoundingClientRect().top - grip) / travel));
    termRef.current?.scrollToLine(Math.round(max * fraction));
  };

  return (
    <div ref={controlsRef} className="terminal-history-controls" title={t('terminal.historyHint')}>
      <button type="button" disabled={disabled} aria-label={t('terminal.historyUp')}
        title={t('terminal.historyUp')} onClick={() => termRef.current?.scrollLines(-3)}>▲</button>
      <div ref={trackRef} className="terminal-history-track" role="scrollbar"
        aria-label={t('terminal.historyPosition')} aria-orientation="vertical"
        aria-valuemin={0} aria-valuemax={max} aria-valuenow={pos} aria-disabled={disabled} tabIndex={disabled ? -1 : 0}
        onPointerDown={(e) => {
          if (disabled || e.button !== 0) return;
          e.preventDefault();
          e.currentTarget.setPointerCapture(e.pointerId);
          const onThumb = (e.target as HTMLElement).closest('.terminal-history-thumb');
          const grip = onThumb ? e.clientY - e.currentTarget.getBoundingClientRect().top - top : thumbHeight / 2;
          dragRef.current = { pointer: e.pointerId, grip };
          toLine(e.clientY, grip);
        }}
        onPointerMove={(e) => { if (dragRef.current?.pointer === e.pointerId) toLine(e.clientY, dragRef.current.grip); }}
        onPointerUp={(e) => { if (dragRef.current?.pointer === e.pointerId) { dragRef.current = null; if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId); } }}
        onKeyDown={(e) => {
          const term = termRef.current;
          const step = { ArrowUp: -3, ArrowDown: 3, PageUp: -rows, PageDown: rows }[e.key];
          if (step !== undefined) { e.preventDefault(); term?.scrollLines(step); }
          else if (e.key === 'Home') { e.preventDefault(); term?.scrollToTop(); }
          else if (e.key === 'End') { e.preventDefault(); term?.scrollToBottom(); }
        }}>
        <div className="terminal-history-thumb" style={{ height: thumbHeight, transform: `translateY(${top}px)` }} />
      </div>
      <button type="button" disabled={disabled} aria-label={t('terminal.historyDown')}
        title={t('terminal.historyDown')} onClick={() => termRef.current?.scrollLines(3)}>▼</button>
    </div>
  );
}

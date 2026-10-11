import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';

export interface HistoryPosition {
  pane: string;
  total: number;
  offset: number;
  rows: number;
  available: boolean;
}

interface Props {
  position: HistoryPosition | null;
  connected: boolean;
  controlsId: string;
  onSeek: (offset: number) => void;
}

/** tmux owns the history; the browser only controls its reported position. */
export default function TerminalHistoryBar({ position, connected, controlsId, onSeek }: Props) {
  const { t } = useT();
  const trackRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ pointer: number; grip: number } | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingRef = useRef<number | null>(null);
  const onSeekRef = useRef(onSeek);
  onSeekRef.current = onSeek;
  const [height, setHeight] = useState(0);
  const [preview, setPreview] = useState<number | null>(null);
  const total = position?.total ?? 0;
  const rows = position?.rows ?? 1;
  const disabled = !connected || !position?.available || total === 0;
  const offset = Math.min(total, Math.max(0, preview ?? position?.offset ?? 0));
  const thumbHeight = Math.min(height, Math.max(28, height * rows / (rows + total)));
  const travel = Math.max(0, height - thumbHeight);
  const top = total ? travel * (1 - offset / total) : 0;

  useEffect(() => {
    const track = trackRef.current;
    if (!track) return;
    const observer = new ResizeObserver(() => setHeight(track.clientHeight));
    observer.observe(track);
    return () => observer.disconnect();
  }, []);

  const cancelPending = () => {
    if (timerRef.current !== null) clearTimeout(timerRef.current);
    timerRef.current = null;
    pendingRef.current = null;
  };
  useEffect(() => () => cancelPending(), []);
  useEffect(() => {
    cancelPending();
    dragRef.current = null;
    setPreview(null);
  }, [disabled, position?.pane]);

  const flush = () => {
    const next = pendingRef.current;
    cancelPending();
    if (next !== null) onSeekRef.current(next);
  };
  const move = (clientY: number) => {
    const track = trackRef.current;
    const drag = dragRef.current;
    if (!track || !drag || !travel) return;
    const fraction = Math.max(0, Math.min(1, (clientY - track.getBoundingClientRect().top - drag.grip) / travel));
    const next = Math.round(total * (1 - fraction));
    setPreview(next);
    pendingRef.current = next;
    // Coalesce pointer events so a long history does not flood the server.
    if (timerRef.current === null) timerRef.current = setTimeout(flush, 60);
  };
  const finish = (event: React.PointerEvent) => {
    if (dragRef.current?.pointer !== event.pointerId) return;
    flush();
    dragRef.current = null;
    setPreview(null);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  };

  return <div ref={trackRef} className="terminal-history-track"
    role="scrollbar" aria-controls={controlsId} aria-label={t('terminal.historyPosition')} aria-orientation="vertical"
    aria-valuemin={0} aria-valuemax={total} aria-valuenow={total - offset}
    aria-disabled={disabled} tabIndex={disabled ? -1 : 0}
    onPointerDown={event => {
      if (disabled || event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.focus();
      event.currentTarget.setPointerCapture(event.pointerId);
      const grip = (event.target as HTMLElement).closest('.terminal-history-thumb')
        ? event.clientY - event.currentTarget.getBoundingClientRect().top - top : thumbHeight / 2;
      dragRef.current = { pointer: event.pointerId, grip };
      move(event.clientY);
    }}
    onPointerMove={event => { if (dragRef.current?.pointer === event.pointerId) move(event.clientY); }}
    onPointerUp={finish} onPointerCancel={finish} onLostPointerCapture={finish}
    onKeyDown={event => {
      if (disabled) return;
      const next = { ArrowUp: offset + 3, ArrowDown: offset - 3,
        PageUp: offset + rows, PageDown: offset - rows, Home: total, End: 0 }[event.key];
      if (next === undefined) return;
      event.preventDefault();
      event.stopPropagation();
      onSeek(Math.max(0, Math.min(total, next)));
    }}>
    <div className="terminal-history-thumb" style={{ height: thumbHeight, transform: `translateY(${top}px)` }} />
  </div>;
}

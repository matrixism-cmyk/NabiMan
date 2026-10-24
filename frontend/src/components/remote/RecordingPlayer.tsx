import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import 'xterm/css/xterm.css';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import { Cast, loadCast } from './recordingsApi';

const SPEEDS = [0.5, 1, 2, 4, 8];
const token = () => sessionStorage.getItem('nabiman_token');

function mmss(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/**
 * Replays a session recording in an xterm terminal, with play/pause, speed
 * and a seek bar. Seeking rebuilds the screen by replaying every frame up to
 * the target instantly, so the terminal state is always correct.
 */
export default function RecordingPlayer({ sid, label, onClose }: { sid: string; label: string; onClose: () => void }) {
  const { t } = useT();
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const cast = useRef<Cast | null>(null);
  const idx = useRef(0);            // next frame to write
  const base = useRef(0);           // virtual ms at the last (re)start
  const wall = useRef(0);           // wall clock at the last (re)start
  const raf = useRef(0);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState('');
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const [pos, setPos] = useState(0);
  const [duration, setDuration] = useState(0);

  // Writes every frame from `idx` whose time is within `virtual` ms.
  const feed = useCallback((virtual: number) => {
    const c = cast.current; const term0 = term.current;
    if (!c || !term0) return;
    while (idx.current < c.frames.length && c.frames[idx.current].t <= virtual) {
      term0.write(c.frames[idx.current].data);
      idx.current += 1;
    }
  }, []);

  const stop = useCallback(() => { cancelAnimationFrame(raf.current); raf.current = 0; }, []);

  const tick = useCallback(() => {
    const c = cast.current;
    if (!c) return;
    const virtual = base.current + (performance.now() - wall.current) * speed;
    feed(virtual);
    setPos(Math.min(virtual, c.duration));
    if (idx.current >= c.frames.length) { setPlaying(false); stop(); return; }
    raf.current = requestAnimationFrame(tick);
  }, [feed, speed, stop]);

  const play = useCallback(() => {
    const c = cast.current;
    if (!c) return;
    if (idx.current >= c.frames.length) { // replay from the start
      term.current?.reset(); idx.current = 0; base.current = 0; setPos(0);
    } else {
      base.current = pos;
    }
    wall.current = performance.now();
    setPlaying(true);
    stop();
    raf.current = requestAnimationFrame(tick);
  }, [pos, stop, tick]);

  const pause = useCallback(() => { setPlaying(false); stop(); base.current = pos; }, [pos, stop]);

  const seek = useCallback((ms: number) => {
    const c = cast.current; const term0 = term.current;
    if (!c || !term0) return;
    stop(); setPlaying(false);
    term0.reset(); idx.current = 0;
    feed(ms);
    base.current = ms; setPos(ms);
  }, [feed, stop]);

  useEffect(() => {
    let live = true;
    const term0 = new Terminal({ fontFamily: 'monospace', fontSize: 13, convertEol: false, scrollback: 2000, disableStdin: true });
    const fit0 = new FitAddon();
    term0.loadAddon(fit0);
    if (host.current) { term0.open(host.current); try { fit0.fit(); } catch { /* not sized yet */ } }
    term.current = term0; fit.current = fit0;
    loadCast(sid, token()).then((c) => {
      if (!live) return;
      cast.current = c;
      try { term0.resize(c.cols, c.rows); } catch { /* ignore */ }
      setDuration(c.duration); setReady(true);
    }).catch((e) => { if (live) setError(e instanceof Error ? e.message : 'error'); });
    return () => { live = false; cancelAnimationFrame(raf.current); term0.dispose(); term.current = null; };
  }, [sid]);

  return (
    <Modal className="rec-player" onClose={onClose} title={`${t('remote.rec.play')} · ${label}`}>
      <div className="rec-screen" ref={host} />
      {error && <p role="alert" className="message">{error}</p>}
      <div className="rec-controls">
        <button type="button" className="btn btn-secondary btn-sm" disabled={!ready} onClick={() => (playing ? pause() : play())}>
          {playing ? `⏸ ${t('remote.rec.pause')}` : `▶ ${t('remote.rec.playBtn')}`}
        </button>
        <input type="range" className="rec-seek" min={0} max={duration || 1} value={pos} disabled={!ready}
          onChange={(e) => seek(Number(e.target.value))} aria-label={t('remote.rec.seek')} />
        <span className="rec-time">{mmss(pos)} / {mmss(duration)}</span>
        <select className="select-input rec-speed" value={speed} disabled={!ready}
          onChange={(e) => { setSpeed(Number(e.target.value)); if (playing) { base.current = pos; wall.current = performance.now(); } }}>
          {SPEEDS.map((s) => <option key={s} value={s}>{s}×</option>)}
        </select>
      </div>
    </Modal>
  );
}

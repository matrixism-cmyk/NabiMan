import React, { RefObject, useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import type { TerminalViewHandle } from './terminalTypes';
import { BarKey, Mods, NO_MODS } from './stickyKeys';
import './mobile-keys.css';

interface Props {
  handle: RefObject<TerminalViewHandle | null>;
  /** Off while the terminal is not taking input (read-only, ended). */
  disabled?: boolean;
}

type Key = { label: string; key: BarKey | string; repeat?: boolean; name?: string };

const KEYS: Key[] = [
  { label: 'Esc', key: 'Escape' },
  { label: 'Tab', key: 'Tab' },
  { label: '←', key: 'ArrowLeft', repeat: true, name: 'left' },
  { label: '↓', key: 'ArrowDown', repeat: true, name: 'down' },
  { label: '↑', key: 'ArrowUp', repeat: true, name: 'up' },
  { label: '→', key: 'ArrowRight', repeat: true, name: 'right' },
  { label: '|', key: '|' }, { label: '~', key: '~' }, { label: '/', key: '/' }, { label: '-', key: '-' },
  { label: '_', key: '_' }, { label: '$', key: '$' }, { label: '&', key: '&' }, { label: '>', key: '>' },
  { label: '*', key: '*' }, { label: '\\', key: '\\' }, { label: '`', key: '`' },
  { label: 'Home', key: 'Home' }, { label: 'End', key: 'End' },
  { label: 'PgUp', key: 'PageUp', repeat: true }, { label: 'PgDn', key: 'PageDown', repeat: true },
];

/** A held arrow repeats after this long, this often. */
const REPEAT_AFTER = 400;
const REPEAT_EVERY = 70;

const typing = () => document.activeElement?.classList.contains('xterm-helper-textarea') ?? false;

/**
 * The keys a phone's keyboard lacks, in a row above it: Esc, Tab, arrows,
 * the symbols a shell needs, and Ctrl and Alt, which hold for the next key
 * typed on the keyboard (Ctrl, then c, is ^C). Tapping the row never takes
 * focus from the terminal, so the keyboard stays where it is.
 */
export default function MobileKeyBar({ handle, disabled }: Props) {
  const { t } = useT();
  const [mods, setMods] = useState<Mods>(NO_MODS);
  const repeat = useRef<{ timer?: number; every?: number; fired?: boolean }>({});
  const stopRepeat = () => {
    window.clearTimeout(repeat.current.timer);
    window.clearInterval(repeat.current.every);
    repeat.current.timer = repeat.current.every = undefined;
  };
  useEffect(() => stopRepeat, []);

  const hold = (next: Mods) => {
    setMods(next);
    handle.current?.armModifiers(next, () => setMods(NO_MODS));
  };
  const press = (key: BarKey | string) => {
    handle.current?.pressKey(key, mods);
    if (mods.ctrl || mods.alt) hold(NO_MODS);
  };
  // Keep focus (and the keyboard) where it is: never let a tap move it.
  const keep = (e: React.PointerEvent | React.MouseEvent) => e.preventDefault();
  // A key types on release, so a finger sliding the row along types nothing
  // (the browser cancels the press when it scrolls). Held, an arrow repeats.
  const down = (k: Key) => (e: React.PointerEvent) => {
    keep(e);
    stopRepeat();
    // Touch sends pointerleave between release and click: what was pressed is kept until the click.
    repeat.current.fired = false;
    if (disabled || !k.repeat) return;
    repeat.current.timer = window.setTimeout(() => {
      repeat.current.fired = true;
      press(k.key);
      repeat.current.every = window.setInterval(() => handle.current?.pressKey(k.key), REPEAT_EVERY);
    }, REPEAT_AFTER);
  };
  const up = (k: Key) => () => {
    const repeated = repeat.current.fired;
    stopRepeat();
    if (!repeated && !disabled) press(k.key);
  };

  const paste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) handle.current?.insertText(text);
    } catch { /* not allowed: the keyboard's own paste still works */ }
  };

  return (
    <div className={`mkeys ${disabled ? 'is-off' : ''}`} role="toolbar" aria-label={t('terminal.keyBar')}
      onContextMenu={(e) => e.preventDefault()}>
      <button type="button" className="mkeys-key mkeys-kbd" tabIndex={-1} onPointerDown={keep} onMouseDown={keep}
        aria-label={t('terminal.keyboard')} title={t('terminal.keyboard')}
        onClick={() => { if (typing()) (document.activeElement as HTMLElement).blur(); else handle.current?.focus(); }}>⌨</button>
      <button type="button" className={`mkeys-key mkeys-mod ${mods.ctrl ? 'is-on' : ''}`} tabIndex={-1}
        aria-pressed={mods.ctrl} onPointerDown={keep} onMouseDown={keep} disabled={disabled}
        onClick={() => hold({ ...mods, ctrl: !mods.ctrl })}>Ctrl</button>
      <button type="button" className={`mkeys-key mkeys-mod ${mods.alt ? 'is-on' : ''}`} tabIndex={-1}
        aria-pressed={mods.alt} onPointerDown={keep} onMouseDown={keep} disabled={disabled}
        onClick={() => hold({ ...mods, alt: !mods.alt })}>Alt</button>
      <div className="mkeys-scroll">
        {KEYS.map((k) => (
          <button key={k.label} type="button" className="mkeys-key" tabIndex={-1} disabled={disabled}
            aria-label={k.name ? t(`terminal.key.${k.name}`) : k.label}
            onPointerDown={down(k)} onPointerCancel={stopRepeat} onPointerLeave={stopRepeat}
            onMouseDown={keep} onClick={up(k)}>{k.label}</button>
        ))}
        <button type="button" className="mkeys-key" tabIndex={-1} onPointerDown={keep} onMouseDown={keep} disabled={disabled}
          aria-label={t('terminal.paste')} title={t('terminal.paste')} onClick={paste}>📋</button>
        <button type="button" className="mkeys-key" tabIndex={-1} onPointerDown={keep} onMouseDown={keep}
          aria-label={t('terminal.zoomOut')} onClick={() => handle.current?.zoomBy(-1)}>A−</button>
        <button type="button" className="mkeys-key" tabIndex={-1} onPointerDown={keep} onMouseDown={keep}
          aria-label={t('terminal.zoomIn')} onClick={() => handle.current?.zoomBy(1)}>A+</button>
      </div>
    </div>
  );
}

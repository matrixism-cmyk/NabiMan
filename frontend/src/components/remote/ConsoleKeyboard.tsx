import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../shared/i18n';
import { charKeysym, inputDiff, KEY, NAMED } from './consoleKeys';

const TOUCH = () => (typeof window !== 'undefined' && window.matchMedia?.('(pointer: coarse)').matches) || navigator.maxTouchPoints > 0;

/** Keys a phone keyboard lacks, on the row above it. Ctrl and Alt hold for the next key. */
const ROW: { label: string; keysym: number }[] = [
  { label: 'Esc', keysym: KEY.ESCAPE }, { label: 'Tab', keysym: KEY.TAB }, { label: 'Win', keysym: KEY.WIN },
  { label: '←', keysym: KEY.LEFT }, { label: '↑', keysym: KEY.UP }, { label: '↓', keysym: KEY.DOWN }, { label: '→', keysym: KEY.RIGHT },
  { label: 'Home', keysym: KEY.HOME }, { label: 'End', keysym: KEY.END }, { label: 'Del', keysym: KEY.DELETE },
  // A phone has no Caps Lock, and a VM left with it on types a password in capitals.
  { label: 'Caps', keysym: KEY.CAPS },
];

/** What was typed shows at the start of the row, the last this many characters. */
const ECHO = 16;

/**
 * A phone's keyboard for a VM console. A touch screen has no key events to
 * catch on the screen itself, so ⌨ focuses a hidden text box, which brings up
 * the phone's keyboard; what is typed there goes to the VM as keystrokes, and
 * a row of keys the phone lacks sits under the screen while it is up.
 *
 * The box itself cannot be seen, and a VM's password field shows only dots,
 * so a phone keyboard left in 한글 mode (or capitalizing the first letter)
 * typed a wrong password with no sign of it. The row shows what was typed,
 * and 🔒 swaps the box for a password box: phones then bring up a plain
 * English keyboard without capitals, corrections or suggestions.
 *
 * ⌨ is a mode, on until it is pressed again. A tap on the screen (to click
 * into a text field) used to put the keyboard away, as the screen took the
 * focus; now the focus comes straight back, and a tap brings the keyboard
 * back up when the phone put it away.
 */
export function useConsoleKeyboard({ send, enabled, keysym = charKeysym, screen }: {
  send: (keysym: number, down: boolean) => void;
  enabled: boolean;
  /** The VM's screen, whose taps keep the keyboard up. */
  screen?: React.RefObject<HTMLElement | null>;
  /** The keysym for a typed character (RDP sends every one as Unicode). */
  keysym?: (cp: number) => number;
}) {
  const { t } = useT();
  const touch = useMemo(TOUCH, []);
  const box = useRef<HTMLTextAreaElement>(null);
  const secretBox = useRef<HTMLInputElement>(null);
  const [secret, setSecret] = useState(false);
  const [typed, setTyped] = useState('');
  const last = useRef('');
  const [open, setOpen] = useState(false);
  // Whether the phone's keyboard is up (one of the boxes has the focus).
  const [up, setUp] = useState(false);
  const [mods, setMods] = useState({ ctrl: false, alt: false });
  const modsRef = useRef(mods);
  modsRef.current = mods;
  // A phone's keyboard covers the bottom of the page without resizing it, so
  // the row of keys rides on top of the keyboard (the visual viewport's bottom).
  const [lift, setLift] = useState(0);
  useEffect(() => {
    const vv = window.visualViewport;
    if (!open || !up || !vv) { setLift(0); return undefined; }
    const place = () => setLift(Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop)));
    place();
    vv.addEventListener('resize', place);
    vv.addEventListener('scroll', place);
    return () => { vv.removeEventListener('resize', place); vv.removeEventListener('scroll', place); };
  }, [open, up]);

  useEffect(() => {
    const el = screen?.current;
    if (!open || !el) return undefined;
    const raise = (e: Event) => {
      if (e.type === 'touchend' && (e as TouchEvent).touches.length) return;
      const target = secret ? secretBox.current : box.current;
      // In the tap itself, or a phone will not raise its keyboard.
      if (target && document.activeElement !== target) target.focus({ preventScroll: true });
    };
    el.addEventListener('touchend', raise, true);
    el.addEventListener('focusin', raise, true);
    return () => { el.removeEventListener('touchend', raise, true); el.removeEventListener('focusin', raise, true); };
  }, [open, secret, screen]);

  const tap = (keysym: number) => {
    const held = [modsRef.current.ctrl && KEY.CTRL, modsRef.current.alt && KEY.ALT].filter(Boolean) as number[];
    held.forEach((m) => send(m, true));
    send(keysym, true); send(keysym, false);
    held.reverse().forEach((m) => send(m, false));
    if (held.length) setMods({ ctrl: false, alt: false });
  };
  const clear = () => {
    if (box.current) box.current.value = '';
    if (secretBox.current) secretBox.current.value = '';
    last.current = ''; setTyped('');
  };

  const onInput = (e: React.FormEvent<HTMLTextAreaElement | HTMLInputElement>) => {
    const el = e.currentTarget;
    const { back, add } = inputDiff(last.current, el.value);
    for (let i = 0; i < back; i += 1) tap(KEY.BACKSPACE);
    for (const ch of add) {
      const cp = ch.codePointAt(0) ?? 0;
      // With Ctrl or Alt held it is a shortcut (Ctrl+C), which wants the plain key.
      const held = modsRef.current.ctrl || modsRef.current.alt;
      tap(ch === '\n' ? KEY.ENTER : held ? charKeysym(cp) : keysym(cp));
    }
    // A finished line starts the box afresh; so does a long one, to keep the comparison short.
    if (add.includes('\n') || el.value.length > 200) clear(); else { last.current = el.value; setTyped(el.value); }
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement | HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    const named = NAMED[e.key];
    if (named) { e.preventDefault(); tap(named); }
    else if (e.key === 'Enter') { e.preventDefault(); tap(KEY.ENTER); clear(); }
    // Nothing left in the box to delete, so no change would reach onInput.
    else if (e.key === 'Backspace' && !e.currentTarget.value) { e.preventDefault(); tap(KEY.BACKSPACE); }
  };
  const toggle = () => {
    if (open && up) { (secret ? secretBox : box).current?.blur(); setOpen(false); return; }
    // On, but the phone put its keyboard away: ⌨ brings it back.
    if (!open) { clear(); setSecret(false); }
    // Focus inside the tap itself, or a phone will not raise its keyboard.
    (open && secret ? secretBox : box).current?.focus({ preventScroll: true });
    setOpen(true);
  };
  // Keep the box focused (and the keyboard up) when a key on the row is pressed.
  const keep = (e: React.PointerEvent) => e.preventDefault();
  // Moving between the two boxes is not putting the keyboard away.
  const blur = (e: React.FocusEvent) => { if (e.relatedTarget !== box.current && e.relatedTarget !== secretBox.current) setUp(false); };
  const focus = () => setUp(true);
  const flip = () => {
    const next = !secret;
    clear(); setSecret(next);
    // In the tap itself, so the phone swaps keyboards instead of closing it.
    (next ? secretBox : box).current?.focus({ preventScroll: true });
  };

  const shown = enabled && touch;
  const button = shown ? (
    <button type="button" className={`btn btn-secondary btn-sm ${open ? 'is-active' : ''}`} onPointerDown={open ? keep : undefined} onClick={toggle}
      title={t('remote.console.keyboardHint')} aria-label={t('remote.console.keyboard')} aria-pressed={open}>⌨</button>
  ) : null;
  const panel = shown ? (
    <div className={`rw-console-keys ${open ? 'is-open' : ''}`}>
      <textarea ref={box} className="rw-console-keybox" aria-label={t('remote.console.keyboard')} autoCapitalize="off" autoComplete="off"
        autoCorrect="off" spellCheck={false} onInput={onInput} onKeyDown={onKeyDown} onFocus={focus} onBlur={blur} />
      <input ref={secretBox} type="password" className="rw-console-keybox" aria-label={t('remote.console.secret')} autoComplete="off"
        autoCapitalize="off" autoCorrect="off" spellCheck={false} onInput={onInput} onKeyDown={onKeyDown} onFocus={focus} onBlur={blur} />
      {open && (
        <div className={`rw-console-keyrow ${lift > 0 ? 'is-lifted' : ''}`} style={lift > 0 ? { bottom: lift } : undefined}>
          <span className="rw-console-typed" aria-live="polite" title={t('remote.console.typedHint')}>
            {secret ? '●'.repeat(Math.min(Array.from(typed).length, ECHO)) : Array.from(typed).slice(-ECHO).join('')}
          </span>
          <button type="button" className={`btn btn-secondary btn-sm ${secret ? 'is-active' : ''}`} onPointerDown={keep} onClick={flip}
            aria-pressed={secret} title={t('remote.console.secretHint')}>🔒 {t('remote.console.secret')}</button>
          {(['ctrl', 'alt'] as const).map((m) => (
            <button key={m} type="button" className={`btn btn-secondary btn-sm ${mods[m] ? 'is-active' : ''}`} onPointerDown={keep}
              aria-pressed={mods[m]} onClick={() => setMods((cur) => ({ ...cur, [m]: !cur[m] }))}>{m === 'ctrl' ? 'Ctrl' : 'Alt'}</button>
          ))}
          {ROW.map((k) => (
            <button key={k.label} type="button" className="btn btn-secondary btn-sm" onPointerDown={keep} onClick={() => tap(k.keysym)}>{k.label}</button>
          ))}
        </div>
      )}
    </div>
  ) : null;
  return { button, panel };
}

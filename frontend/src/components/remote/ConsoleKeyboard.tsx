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

/**
 * A phone's keyboard for a VM console. A touch screen has no key events to
 * catch on the screen itself, so ⌨ focuses a hidden text box, which brings up
 * the phone's keyboard; what is typed there goes to the VM as keystrokes, and
 * a row of keys the phone lacks sits under the screen while it is up.
 */
export function useConsoleKeyboard({ send, enabled, keysym = charKeysym }: {
  send: (keysym: number, down: boolean) => void;
  enabled: boolean;
  /** The keysym for a typed character (RDP sends every one as Unicode). */
  keysym?: (cp: number) => number;
}) {
  const { t } = useT();
  const touch = useMemo(TOUCH, []);
  const box = useRef<HTMLTextAreaElement>(null);
  const last = useRef('');
  const [open, setOpen] = useState(false);
  const [mods, setMods] = useState({ ctrl: false, alt: false });
  const modsRef = useRef(mods);
  modsRef.current = mods;
  // A phone's keyboard covers the bottom of the page without resizing it, so
  // the row of keys rides on top of the keyboard (the visual viewport's bottom).
  const [lift, setLift] = useState(0);
  useEffect(() => {
    const vv = window.visualViewport;
    if (!open || !vv) { setLift(0); return undefined; }
    const place = () => setLift(Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop)));
    place();
    vv.addEventListener('resize', place);
    vv.addEventListener('scroll', place);
    return () => { vv.removeEventListener('resize', place); vv.removeEventListener('scroll', place); };
  }, [open]);

  const tap = (keysym: number) => {
    const held = [modsRef.current.ctrl && KEY.CTRL, modsRef.current.alt && KEY.ALT].filter(Boolean) as number[];
    held.forEach((m) => send(m, true));
    send(keysym, true); send(keysym, false);
    held.reverse().forEach((m) => send(m, false));
    if (held.length) setMods({ ctrl: false, alt: false });
  };
  const clear = () => { if (box.current) box.current.value = ''; last.current = ''; };

  const onInput = () => {
    const el = box.current; if (!el) return;
    const { back, add } = inputDiff(last.current, el.value);
    for (let i = 0; i < back; i += 1) tap(KEY.BACKSPACE);
    for (const ch of add) {
      const cp = ch.codePointAt(0) ?? 0;
      // With Ctrl or Alt held it is a shortcut (Ctrl+C), which wants the plain key.
      const held = modsRef.current.ctrl || modsRef.current.alt;
      tap(ch === '\n' ? KEY.ENTER : held ? charKeysym(cp) : keysym(cp));
    }
    // A finished line starts the box afresh; so does a long one, to keep the comparison short.
    if (add.includes('\n') || el.value.length > 200) clear(); else last.current = el.value;
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    const named = NAMED[e.key];
    if (named) { e.preventDefault(); tap(named); }
    else if (e.key === 'Enter') { e.preventDefault(); tap(KEY.ENTER); clear(); }
    // Nothing left in the box to delete, so no change would reach onInput.
    else if (e.key === 'Backspace' && !e.currentTarget.value) { e.preventDefault(); tap(KEY.BACKSPACE); }
  };
  const toggle = () => {
    if (open) { box.current?.blur(); setOpen(false); return; }
    clear();
    // Focus inside the tap itself, or a phone will not raise its keyboard.
    box.current?.focus({ preventScroll: true });
    setOpen(true);
  };
  // Keep the box focused (and the keyboard up) when a key on the row is pressed.
  const keep = (e: React.PointerEvent) => e.preventDefault();

  const shown = enabled && touch;
  const button = shown ? (
    <button type="button" className={`btn btn-secondary btn-sm ${open ? 'is-active' : ''}`} onPointerDown={open ? keep : undefined} onClick={toggle}
      title={t('remote.console.keyboardHint')} aria-label={t('remote.console.keyboard')} aria-pressed={open}>⌨</button>
  ) : null;
  const panel = shown ? (
    <div className={`rw-console-keys ${open ? 'is-open' : ''}`}>
      <textarea ref={box} className="rw-console-keybox" aria-label={t('remote.console.keyboard')} autoCapitalize="off" autoComplete="off"
        autoCorrect="off" spellCheck={false} onInput={onInput} onKeyDown={onKeyDown} onBlur={() => setOpen(false)} />
      {open && (
        <div className={`rw-console-keyrow ${lift > 0 ? 'is-lifted' : ''}`} style={lift > 0 ? { bottom: lift } : undefined}>
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

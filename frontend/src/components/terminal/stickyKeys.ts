/**
 * Keys a phone's keyboard does not have, as a terminal sends them, and Ctrl
 * or Alt held for the next key: tapped on the key bar, they apply to whatever
 * is typed next on the soft keyboard (Ctrl, then "c", sends ^C).
 */

export interface Mods { ctrl: boolean; alt: boolean }
export const NO_MODS: Mods = { ctrl: false, alt: false };

export type BarKey = 'Escape' | 'Tab' | 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight'
  | 'Home' | 'End' | 'PageUp' | 'PageDown';

const BAR_KEYS: ReadonlySet<string> = new Set(['Escape', 'Tab', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown']);
export const isBarKey = (key: string): key is BarKey => BAR_KEYS.has(key);

const LETTER: Partial<Record<BarKey, string>> = {
  ArrowUp: 'A', ArrowDown: 'B', ArrowRight: 'C', ArrowLeft: 'D', Home: 'H', End: 'F',
};

/**
 * What `key` sends, as xterm itself would: arrows follow the application
 * cursor mode programs like vim and less turn on, and modifiers go into the
 * sequence the xterm way (`ESC [1;5A` is Ctrl+↑).
 */
export function keySequence(key: BarKey, appCursor: boolean, mods: Mods = NO_MODS): string {
  const m = 1 + (mods.alt ? 2 : 0) + (mods.ctrl ? 4 : 0);
  const letter = LETTER[key];
  if (letter) {
    if (m > 1) return `\x1b[1;${m}${letter}`;
    return appCursor ? `\x1bO${letter}` : `\x1b[${letter}`;
  }
  if (key === 'PageUp' || key === 'PageDown') {
    const n = key === 'PageUp' ? 5 : 6;
    return m > 1 ? `\x1b[${n};${m}~` : `\x1b[${n}~`;
  }
  const plain = key === 'Escape' ? '\x1b' : '\t';
  return mods.alt ? `\x1b${plain}` : plain;
}

const CONTROL: Record<string, string> = {
  ' ': '\x00', '@': '\x00', '2': '\x00', '[': '\x1b', '3': '\x1b', '\\': '\x1c', '4': '\x1c',
  ']': '\x1d', '5': '\x1d', '^': '\x1e', '6': '\x1e', '_': '\x1f', '-': '\x1f', '/': '\x1f', '7': '\x1f',
  '?': '\x7f', '8': '\x7f',
};

/** A character typed with Ctrl and/or Alt held, as a terminal sends it. */
export function withMods(data: string, mods: Mods): string {
  let out = data;
  if (mods.ctrl && data.length === 1) {
    if (/^[a-z]$/i.test(data)) out = String.fromCharCode(data.toUpperCase().charCodeAt(0) - 64);
    else if (CONTROL[data] !== undefined) out = CONTROL[data];
  }
  return mods.alt ? `\x1b${out}` : out;
}

const armed = new WeakMap<object, { mods: Mods; used: () => void }>();

/** Holds `mods` for the next thing typed into `term`; `used` is told when it goes. */
export function armModifiers(term: object, mods: Mods, used: () => void): void {
  if (mods.ctrl || mods.alt) armed.set(term, { mods, used });
  else armed.delete(term);
}

/** What is typed, with any armed modifiers applied (and then let go). */
export function applyArmed(term: object, data: string): string {
  const held = armed.get(term);
  if (!held) return data;
  armed.delete(term);
  held.used();
  return withMods(data, held.mods);
}

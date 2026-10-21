import type { Terminal } from 'xterm';
import './terminal-fonts.css';

/** One-cell circled digits come first, whatever the font (see terminal-symbols.css). */
const DIGITS = "'NabiMan Circled Digits'";
const SYSTEM = "'Fira Code', 'Cascadia Code', 'JetBrains Mono', 'Menlo', 'Consolas', monospace";

export interface TerminalFont {
  id: string;
  /** Shown in settings as is: font names are not translated. */
  label: string;
  /** The face that sets the cell size, to wait for before measuring. */
  face: string | null;
  family: string;
}

export const TERMINAL_FONTS: TerminalFont[] = [
  { id: 'd2coding', label: 'D2Coding', face: 'NabiMan D2Coding', family: `${DIGITS}, 'NabiMan D2Coding', monospace` },
  { id: 'jetbrains', label: 'JetBrains Mono + D2Coding', face: 'NabiMan JetBrains Mono', family: `${DIGITS}, 'NabiMan JetBrains Mono', 'NabiMan Hangul', monospace` },
  { id: 'fira', label: 'Fira Code + D2Coding', face: 'NabiMan Fira Code', family: `${DIGITS}, 'NabiMan Fira Code', 'NabiMan Hangul', monospace` },
  { id: 'cascadia', label: 'Cascadia Mono + D2Coding', face: 'NabiMan Cascadia Mono', family: `${DIGITS}, 'NabiMan Cascadia Mono', 'NabiMan Hangul', monospace` },
  { id: 'nanum', label: '나눔고딕코딩', face: 'NabiMan Nanum Gothic Coding', family: `${DIGITS}, 'NabiMan Nanum Gothic Coding', monospace` },
  { id: 'system', label: 'system', face: null, family: `${DIGITS}, ${SYSTEM}` },
];

export const DEFAULT_FONT = 'd2coding';

export function fontOf(id: string | undefined): TerminalFont {
  return TERMINAL_FONTS.find((f) => f.id === id) || TERMINAL_FONTS[0];
}

/**
 * Puts `id`'s font on `term` once it has loaded, so the cells are measured
 * with it rather than with whatever stood in while it downloaded; `ready`
 * re-fits the pane to the new cell size.
 */
export function applyTerminalFont(term: Terminal, id: string | undefined, ready: () => void, alive: () => boolean) {
  const font = fontOf(id);
  const size = term.options.fontSize || 14;
  const loads = [document.fonts?.load(`${size}px "NabiMan Circled Digits"`, '①')];
  if (font.face) loads.push(document.fonts?.load(`${size}px "${font.face}"`, 'W'));
  term.options.fontFamily = font.family;
  if (!document.fonts) return;
  void Promise.all(loads).catch(() => undefined).then(() => {
    if (!alive()) return;
    // A different string makes xterm measure the cell again, now with the real font.
    term.options.fontFamily = `${font.family} `;
    term.clearTextureAtlas();
    term.refresh(0, term.rows - 1);
    ready();
  });
}

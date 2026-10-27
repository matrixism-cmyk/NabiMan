/**
 * Typing into a VM from a phone's own keyboard. The keyboard types into a
 * hidden text box; what changed there since the last look is sent as
 * keystrokes. Korean keyboards rewrite the syllable being built (ㄱ → 가 → 각),
 * so a change can take characters back as well as add them.
 */

export const KEY = {
  BACKSPACE: 0xff08, TAB: 0xff09, ENTER: 0xff0d, ESCAPE: 0xff1b, DELETE: 0xffff,
  HOME: 0xff50, LEFT: 0xff51, UP: 0xff52, RIGHT: 0xff53, DOWN: 0xff54, END: 0xff57,
  CTRL: 0xffe3, ALT: 0xffe9, WIN: 0xffeb,
};

/** The X11 keysym for a typed character: Latin-1 as is, the rest as Unicode keysyms. */
export const charKeysym = (cp: number): number => (cp < 0x100 ? cp : 0x01000000 + cp);

/** How the text box went from `before` to `after`: characters taken back, then typed. */
export function inputDiff(before: string, after: string): { back: number; add: string } {
  const a = Array.from(before), b = Array.from(after);
  let same = 0;
  while (same < a.length && same < b.length && a[same] === b[same]) same += 1;
  return { back: a.length - same, add: b.slice(same).join('') };
}

/** Keys a hardware or phone keyboard reports by name, which never reach the text box. */
export const NAMED: Record<string, number> = {
  Escape: KEY.ESCAPE, Tab: KEY.TAB, Delete: KEY.DELETE, Home: KEY.HOME, End: KEY.END,
  ArrowLeft: KEY.LEFT, ArrowUp: KEY.UP, ArrowRight: KEY.RIGHT, ArrowDown: KEY.DOWN,
};

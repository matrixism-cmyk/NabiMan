/**
 * Keys guacd cannot send as they are. guacd 1.3.0 has no Korean keymap, so the
 * 한/영 (Hangul) and 한자 (Hangul_Hanja) keysyms are dropped before they reach
 * the RDP server. A Korean Windows reads right Alt as 한/영 and right Ctrl as
 * 한자 (the standard 101-key "type 1" layout), and both are in guacd's base
 * keymap — so those are sent instead.
 */
export const HANGUL = 0xff31;
export const HANJA = 0xff34;
export const ALT_R = 0xffea;
export const CONTROL_R = 0xffe4;

const SUBSTITUTE: Record<number, number> = { [HANGUL]: ALT_R, [HANJA]: CONTROL_R };

/**
 * The keysym to send for a pressed key, and whether it is sent as a tap. A
 * toggle key is tapped on press and its release ignored: a browser may not
 * report the release of 한/영, and a held right Alt would stick.
 */
export function remoteKey(keysym: number): { keysym: number; tap: boolean } {
  const sub = SUBSTITUTE[keysym];
  return sub ? { keysym: sub, tap: true } : { keysym, tap: false };
}

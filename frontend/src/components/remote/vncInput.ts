import RFB from '@novnc/novnc/lib/rfb.js';

const ENTER = 0xff0d;
const TAB = 0xff09;
const SHIFT = 0xffe1;

/** What a US keyboard types with Shift held. QEMU's VNC server turns a keysym
 *  into the key that carries it and does not press Shift itself, so '!' sent
 *  alone arrives as '1' and 'A' as 'a' — a password with either never matched. */
const SHIFTED = '~!@#$%^&*()_+{}|:"<>?';
export const needsShift = (keysym: number): boolean =>
  (keysym >= 0x41 && keysym <= 0x5a) || (keysym < 0x80 && SHIFTED.includes(String.fromCharCode(keysym)));

/** One key going down or up, with Shift held around it where the character needs it. */
export function sendKeysym(rfb: RFB, keysym: number, down: boolean): void {
  const shift = needsShift(keysym);
  if (shift && down) rfb.sendKey(SHIFT, null, true);
  rfb.sendKey(keysym, null, down);
  if (shift && !down) rfb.sendKey(SHIFT, null, false);
}

const tap = (rfb: RFB, keysym: number) => { sendKeysym(rfb, keysym, true); sendKeysym(rfb, keysym, false); };

/** Types `text` into the VM as keystrokes — works even when the guest has no
 *  clipboard integration. Reliable for ASCII; IME characters (e.g. Hangul) may
 *  not map on servers without extended keysym support. */
export function typeText(rfb: RFB, text: string): void {
  for (const ch of text) {
    if (ch === '\n' || ch === '\r') { tap(rfb, ENTER); continue; }
    if (ch === '\t') { tap(rfb, TAB); continue; }
    if (ch === '\x7f' || ch === '\b') { tap(rfb, 0xff08); continue; }
    const cp = ch.codePointAt(0) ?? 0;
    if (!cp) continue;
    tap(rfb, cp < 0x100 ? cp : 0x01000000 + cp);
  }
}

import RFB from '@novnc/novnc/lib/rfb.js';

const ENTER = 0xff0d;
const TAB = 0xff09;

/** Types `text` into the VM as keystrokes — works even when the guest has no
 *  clipboard integration. Reliable for ASCII; IME characters (e.g. Hangul) may
 *  not map on servers without extended keysym support. */
export function typeText(rfb: RFB, text: string): void {
  for (const ch of text) {
    if (ch === '\n' || ch === '\r') { rfb.sendKey(ENTER, null); continue; }
    if (ch === '\t') { rfb.sendKey(TAB, null); continue; }
    const cp = ch.codePointAt(0) ?? 0;
    if (!cp) continue;
    rfb.sendKey(cp < 0x100 ? cp : 0x01000000 + cp, null);
  }
}

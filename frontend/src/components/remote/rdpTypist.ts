import Guacamole from '../../vendor/guacamole-common.js';
import { KEY } from './consoleKeys';

/** How long the VM gets to take the new clipboard before Ctrl+V, and the app to read it after. */
const BEFORE_PASTE_MS = 200;
const AFTER_PASTE_MS = 300;

const sleep = (ms: number) => new Promise<void>((done) => { window.setTimeout(done, ms); });

/**
 * Dictation into an RDP VM. Typed as keys, a Korean Windows in 한글 mode turns
 * letters into jamo (hello → ㅗ디ㅣㅐ), and some text boxes drop a repeated
 * character sent as Unicode (hello → helo); pasted through the VM's
 * clipboard, the text lands as it was said whatever the input mode. Enter
 * and Backspace stay keys. Each step waits for the one before, so a spoken
 * "엔터" comes after the text it ends.
 */
export function rdpTypist(client: () => any) {
  let chain: Promise<void> = Promise.resolve();
  const later = (step: () => Promise<void> | void) => { chain = chain.then(step).catch(() => undefined); };
  const tap = (c: any, keysym: number) => { c.sendKeyEvent(1, keysym); c.sendKeyEvent(0, keysym); };

  return {
    text(text: string) {
      if (!text) return;
      later(async () => {
        const c = client(); if (!c) return;
        const writer = new Guacamole.StringWriter(c.createClipboardStream('text/plain'));
        writer.sendText(text); writer.sendEnd();
        await sleep(BEFORE_PASTE_MS);
        c.sendKeyEvent(1, KEY.CTRL); tap(c, 0x76); c.sendKeyEvent(0, KEY.CTRL);
        // The app asks for the clipboard after Ctrl+V; the next text must not replace it first.
        await sleep(AFTER_PASTE_MS);
      });
    },
    keys(keys: string) {
      later(() => {
        const c = client(); if (!c) return;
        for (const ch of keys) {
          if (ch === '\r' || ch === '\n') tap(c, KEY.ENTER);
          else if (ch === '\x7f' || ch === '\b') tap(c, KEY.BACKSPACE);
          else tap(c, 0x01000000 + (ch.codePointAt(0) ?? 0));
        }
      });
    },
  };
}

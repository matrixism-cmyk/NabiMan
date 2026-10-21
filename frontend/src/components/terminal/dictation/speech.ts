/**
 * Turning live transcription into keystrokes: only text that has become
 * final is typed, once; the spoken word "엔터" (or "Enter") presses Enter
 * instead of being typed; and microphone audio is cut down to the 16 kHz
 * 16-bit mono the speech service takes.
 */

/** The words that press Enter, standing on their own, with any trailing punctuation. */
const ENTER = /(^|\s)(엔터키?|enter)[.,!?。、…]*(?=\s|$)/giu;

/** A keystroke-level step: type text, press Enter, or erase characters just typed. */
export type Action = { type: 'text'; text: string } | { type: 'enter' } | { type: 'erase'; count: number };

const chars = (text: string) => Array.from(text).length;

function sharedStart(a: string, b: string): number {
  let i = 0;
  const end = Math.min(a.length, b.length);
  while (i < end && a[i] === b[i]) i += 1;
  return i;
}

/**
 * Keeps the prompt in step with a transcript that grows and, while it is
 * tentative, gets corrected. Each call brings the screen up to the
 * transcript so far: new words are typed; a spoken "엔터" (or "Enter") is
 * not typed but pressed; a correction to words already typed on the current
 * line erases them and types the new ones. A line ended with Enter has run,
 * so it is never touched again — a correction there only adds what follows.
 */
export class Typist {
  /** Enters pressed so far: the lines that ran. */
  private enters = 0;
  /** The current line as heard and as far as it is on screen. */
  private lineHeard = '';
  /** What the current line has typed, and the leading spaces it left out. */
  private lineSent = '';
  private skipped = 0;

  /** Where the current line starts in `target`: after the Enters already pressed. */
  private lineStart(target: string): number | null {
    if (!this.enters) return 0;
    const m = Array.from(target.matchAll(ENTER))[this.enters - 1];
    return m ? (m.index ?? 0) + m[0].length : null;
  }

  to(target: string): Action[] {
    const out: Action[] = [];
    const cut = this.lineStart(target);
    // The service took back an Enter that was pressed: what ran, ran.
    if (cut === null) return out;
    const line = target.slice(cut);
    const same = sharedStart(this.lineHeard, line);
    if (same < this.lineHeard.length) {
      // Corrected within the current line: take back what changed.
      const keep = Math.max(0, same - this.skipped);
      const erase = chars(this.lineSent.slice(keep));
      if (erase) out.push({ type: 'erase', count: erase });
      this.lineSent = this.lineSent.slice(0, keep);
      this.skipped = Math.min(this.skipped, same);
      this.lineHeard = this.lineHeard.slice(0, same);
    }
    const fresh = line.slice(this.lineHeard.length);
    let at = 0;
    for (const m of Array.from(fresh.matchAll(ENTER))) {
      const start = (m.index ?? 0) + m[1].length;
      this.send(fresh.slice(at, start).replace(/\s+$/, ''), out);
      out.push({ type: 'enter' });
      at = (m.index ?? 0) + m[0].length;
      this.enters += 1;
      this.lineHeard = '';
      this.lineSent = '';
      this.skipped = 0;
    }
    const rest = fresh.slice(at);
    this.send(rest, out);
    this.lineHeard += rest;
    return out;
  }

  /** What has been heard but is not on screen yet, for showing. */
  pending(target: string): string {
    const cut = this.lineStart(target);
    return cut === null ? '' : target.slice(cut).slice(this.lineHeard.length).trim();
  }

  /** Types `text` on the current line; spaces a line would start with are left out. */
  private send(text: string, out: Action[]) {
    let t = text;
    if (!this.lineSent) {
      const lead = t.length - t.replace(/^\s+/, '').length;
      this.skipped += lead;
      t = t.slice(lead);
    }
    if (!t) return;
    out.push({ type: 'text', text: t });
    this.lineSent += t;
  }
}

/** The text a final message settles on: committed, then the last partial. */
/**
 * One finished utterance — the service heard the speaker stop — as
 * keystrokes: its words, Enter wherever "엔터" was said, and with
 * `enterAtEnd` (sending on a pause) Enter after the last words too.
 * `lineHasText` says whether the current line already has words, so the new
 * ones are set apart from them by a space.
 */
export function utteranceActions(text: string, enterAtEnd: boolean, lineHasText: boolean): Action[] {
  const out: Action[] = [];
  let line = lineHasText;
  let last = 0;
  const words = (piece: string) => {
    const t = piece.trim();
    if (!t) return false;
    out.push({ type: 'text', text: line ? ` ${t}` : t });
    line = true;
    return true;
  };
  for (const m of Array.from(text.matchAll(ENTER))) {
    words(text.slice(last, m.index));
    out.push({ type: 'enter' });
    line = false;
    last = (m.index ?? 0) + m[0].length;
  }
  if (words(text.slice(last)) && enterAtEnd) out.push({ type: 'enter' });
  return out;
}

export function finalText(committed: string, partial: string): string {
  if (!partial) return committed;
  if (!committed) return partial;
  return /\s$/.test(committed) || /^\s/.test(partial) ? committed + partial : `${committed} ${partial}`;
}

/**
 * From the microphone's rate (usually 48 kHz) to 16 kHz 16-bit, averaging
 * each output sample's span of input, and in 0.1 s frames as the service
 * asks. Keeps its place between calls.
 */
export class Resampler {
  private readonly step: number;
  private pos = 0;
  private sum = 0;
  private count = 0;
  private frame = new Int16Array(1600);
  private filled = 0;

  constructor(fromRate: number, private readonly toRate = 16000) {
    this.step = fromRate / toRate;
  }

  /** Feeds samples; returns the whole frames that are ready. */
  push(samples: Float32Array): ArrayBuffer[] {
    const frames: ArrayBuffer[] = [];
    for (let i = 0; i < samples.length; i += 1) {
      this.sum += samples[i];
      this.count += 1;
      this.pos += 1;
      if (this.pos >= this.step) {
        this.pos -= this.step;
        const v = Math.max(-1, Math.min(1, this.sum / this.count));
        this.frame[this.filled] = v < 0 ? v * 0x8000 : v * 0x7fff;
        this.filled += 1;
        this.sum = 0;
        this.count = 0;
        if (this.filled === this.frame.length) {
          frames.push(this.frame.buffer.slice(0));
          this.filled = 0;
        }
      }
    }
    return frames;
  }
}

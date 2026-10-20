/**
 * An answer, read aloud while it is still being written: the text that
 * streams in is cut into sentences as each one ends, and each is cleaned of
 * what only makes sense on screen (markdown, code, links).
 */

/** Where a sentence ends: its mark, then a space or a line break. */
const END = /[.!?。！？…]+["')\]」』]*(?=\s)|\n+/g;
/** Sentences shorter than this go together with the next (fewer, smoother requests). */
const SHORT = 12;
/** A run with no sentence end is cut at a pause past this many characters. */
const LONG = 220;

export class Sentences {
  private buf = '';
  private said = 0;

  /** Text that came in; the sentences it completed. */
  push(delta: string): string[] {
    this.buf += delta;
    const out: string[] = [];
    let start = 0;
    END.lastIndex = 0;
    for (let m = END.exec(this.buf); m; m = END.exec(this.buf)) {
      const end = m.index + m[0].length;
      const piece = this.buf.slice(start, end);
      // The first sentence goes as soon as it can; later short ones wait for company.
      if (piece.trim().length < SHORT && this.said > 0) continue;
      out.push(piece);
      start = end;
    }
    this.buf = this.buf.slice(start);
    if (this.buf.length > LONG) {
      const cut = Math.max(this.buf.lastIndexOf(', ', LONG), this.buf.lastIndexOf(' ', LONG));
      if (cut > SHORT) { out.push(this.buf.slice(0, cut + 1)); this.buf = this.buf.slice(cut + 1); }
    }
    const spoken = out.map(speakable).filter(Boolean);
    this.said += spoken.length;
    return spoken;
  }

  /** The answer ended: whatever is left. */
  flush(): string[] {
    const rest = speakable(this.buf);
    this.buf = '';
    return rest ? [rest] : [];
  }
}

/** What of `text` should be heard: no markdown, code or addresses. */
export function speakable(text: string): string {
  return text
    .replace(/```[\s\S]*?(```|$)/g, ' ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/https?:\/\/\S+/g, ' ')
    .replace(/^\s{0,3}#{1,6}\s+/gm, '')
    .replace(/^\s*(?:[-*+•]|\d+[.)])\s+/gm, '')
    .replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, '$1$2')
    .replace(/(^|\s)\*([^*\n]+)\*(?=\s|$)/g, '$1$2')
    .replace(/^\s*>\s?/gm, '')
    .replace(/\|/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

/** Turning a file's bytes into lines to show: encoding, binary files, colour. */

export type Encoding = 'auto' | 'utf-8' | 'euc-kr' | 'shift_jis' | 'iso-8859-1';
export const ENCODINGS: Encoding[] = ['auto', 'utf-8', 'euc-kr', 'shift_jis', 'iso-8859-1'];

/** A NUL byte near the start means a binary file, as `grep` and `git` decide it. */
export function looksBinary(bytes: Uint8Array): boolean {
  const end = Math.min(bytes.length, 8000);
  for (let i = 0; i < end; i += 1) if (bytes[i] === 0) return true;
  return false;
}

/**
 * The text and the encoding it was read as. `auto` takes UTF-8 when the
 * bytes are valid UTF-8 and otherwise EUC-KR, which is what older Korean
 * files on Linux and Windows servers are mostly in.
 */
export function decode(bytes: Uint8Array, encoding: Encoding): { text: string; encoding: Exclude<Encoding, 'auto'> } {
  let used: Exclude<Encoding, 'auto'> = encoding === 'auto' ? 'utf-8' : encoding;
  let text: string;
  try {
    text = new TextDecoder(used, { fatal: encoding === 'auto' }).decode(bytes);
  } catch {
    used = 'euc-kr';
    text = new TextDecoder(used).decode(bytes);
  }
  return { text: text.replace(/\r\n?/g, '\n'), encoding: used };
}

/** The lines of a text; a final newline does not make an empty last line. */
export function linesOf(text: string): string[] {
  const lines = text.split('\n');
  if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
  return lines;
}

/**
 * highlight.js HTML split into one string per line, each with its spans
 * closed at the end and reopened on the next, so a comment or string that
 * runs over several lines keeps its colour on every one of them.
 */
export function splitHighlighted(html: string): string[] {
  const out: string[] = [];
  const open: string[] = [];
  let line = '';
  const tag = /<span[^>]*>|<\/span>|\n/g;
  let at = 0;
  let m: RegExpExecArray | null;
  while ((m = tag.exec(html))) {
    line += html.slice(at, m.index);
    at = m.index + m[0].length;
    if (m[0] === '\n') {
      out.push(line + '</span>'.repeat(open.length));
      line = open.join('');
    } else if (m[0] === '</span>') {
      open.pop();
      line += m[0];
    } else {
      open.push(m[0]);
      line += m[0];
    }
  }
  line += html.slice(at);
  out.push(line);
  if (out.length > 1 && out[out.length - 1] === '') out.pop();
  return out;
}

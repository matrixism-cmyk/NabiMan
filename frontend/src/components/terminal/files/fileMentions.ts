import type { Terminal } from 'xterm';
import { pathsOnRow } from './fileLines';

/** How far back in the terminal a name's full path is looked for. */
export const MENTION_ROWS = 4000;
const MAX_MENTIONS = 4;

/**
 * The full paths printed elsewhere in the terminal that end in `name`, the
 * nearest to the bottom first.
 *
 * A name often stands alone — Claude's file card shows `[file] report.pdf`
 * for a file that is not in the session's own folder — while the path it came
 * from was printed a little earlier (the command that made it, the line that
 * announced it). That path is tried when the folders a bare name is looked
 * for in do not have it. Like every printed path it only becomes a folder to
 * list through the file panel's own, permission-checked request.
 */
export function mentionedPaths(term: Terminal, name: string): string[] {
  const base = name.slice(name.lastIndexOf('/') + 1);
  if (base.length < 3) return [];
  const buffer = term.buffer.active;
  // The end of the name is on the last row of a wrapped path: a cheap test before the row is parsed.
  const tail = base.slice(-Math.min(base.length, 8));
  const out: string[] = [];
  const from = buffer.length - 1;
  for (let y = from; y >= 0 && y > from - MENTION_ROWS && out.length < MAX_MENTIONS; y -= 1) {
    const row = buffer.getLine(y)?.translateToString(true) || '';
    if (!row.includes(tail)) continue;
    for (const found of pathsOnRow(term, y)) {
      if ((found.kind === 'absolute' || found.kind === 'home') && found.path.endsWith(`/${base}`) && !out.includes(found.path)) out.push(found.path);
    }
  }
  return out.slice(0, MAX_MENTIONS);
}

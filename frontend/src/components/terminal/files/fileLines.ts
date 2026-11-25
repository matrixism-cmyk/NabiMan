import type { IBuffer, IBufferLine, IBufferRange, Terminal } from 'xterm';
import { findPaths, PathMatch } from './filePaths';

/**
 * Paths on a terminal's screen, with where they are drawn.
 *
 * A path can run over several rows. The terminal marks the rows it wrapped
 * itself; a program that draws its own screen (Claude Code, inside tmux)
 * breaks long words with a plain line end instead, so a row filled to the
 * edge is also read together with the next one (`loose`). Reading rows
 * together that do not belong together costs nothing: the joined text is then
 * not a file, and the plain reading of each row is kept as well.
 */
export interface ScreenPath extends PathMatch {
  /** Cells the path covers, as xterm counts them: from 1, end included. */
  range: IBufferRange;
}

/** Rows read together at most, either way from the one asked about. */
const MAX_ROWS = 12;
/** A row whose text ends this close to the right edge counts as filled. */
const EDGE_CELLS = 3;

interface Cell { y: number; x: number; width: number }
interface Logical { text: string; cells: Cell[] }

/** The last cell of a row that holds anything, or -1 for an empty row. */
function lastCell(line: IBufferLine, cols: number): number {
  for (let x = Math.min(cols, line.length) - 1; x >= 0; x--) {
    const cell = line.getCell(x);
    if (cell && cell.getWidth() > 0 && cell.getChars().trim()) return x;
  }
  return -1;
}

/** Whether row `y` carries on from the row above it. */
export function continues(buffer: IBuffer, cols: number, y: number, loose: boolean): boolean {
  const line = buffer.getLine(y);
  const above = buffer.getLine(y - 1);
  if (!line || !above) return false;
  if (line.isWrapped) return true;
  return loose && lastCell(above, cols) >= cols - EDGE_CELLS && lastCell(line, cols) >= 0;
}

/** The rows around `y` that read as one line, as text, with the cell each character is in. */
function logicalLine(buffer: IBuffer, cols: number, y: number, loose: boolean): Logical {
  let first = y;
  let last = y;
  while (y - first < MAX_ROWS && continues(buffer, cols, first, loose)) first -= 1;
  while (last - y < MAX_ROWS && continues(buffer, cols, last + 1, loose)) last += 1;
  let text = '';
  const cells: Cell[] = [];
  for (let row = first; row <= last; row++) {
    const line = buffer.getLine(row);
    if (!line) continue;
    const end = lastCell(line, cols);
    // A row the terminal wrapped is whole to its edge, spaces included; one
    // joined by guess starts after its indent and ends at its last character.
    const wrapped = line.isWrapped;
    const carriesOn = row < last && Boolean(buffer.getLine(row + 1)?.isWrapped);
    let started = row === first || wrapped;
    for (let x = 0; x < Math.min(cols, line.length); x++) {
      const cell = line.getCell(x);
      if (!cell || cell.getWidth() === 0) continue;
      const chars = cell.getChars();
      if (x > end && !(carriesOn && chars)) break;
      if (!started && !chars.trim()) continue;
      started = true;
      for (const unit of Array.from(chars || ' ')) {
        for (let i = 0; i < unit.length; i++) cells.push({ y: row, x, width: cell.getWidth() });
        text += unit;
      }
    }
  }
  return { text, cells };
}

const sameRange = (a: IBufferRange, b: IBufferRange) =>
  a.start.x === b.start.x && a.start.y === b.start.y && a.end.x === b.end.x && a.end.y === b.end.y;

/** The paths that touch buffer row `y` (from 0), the longest first. */
export function pathsOnRow(term: Terminal, y: number): ScreenPath[] {
  const buffer = term.buffer.active;
  const out: ScreenPath[] = [];
  let strict = '';
  for (const loose of [false, true]) {
    const logical = logicalLine(buffer, term.cols, y, loose);
    if (loose && logical.text === strict) break;
    strict = logical.text;
    for (const match of findPaths(logical.text)) {
      const from = logical.cells[match.start];
      const to = logical.cells[match.end - 1];
      if (!from || !to || from.y > y || to.y < y) continue;
      const range = { start: { x: from.x + 1, y: from.y + 1 }, end: { x: to.x + to.width, y: to.y + 1 } };
      if (!out.some((p) => p.path === match.path && sameRange(p.range, range))) out.push({ ...match, range });
    }
  }
  return out.sort((a, b) => b.path.length - a.path.length);
}

const inRange = (r: IBufferRange, x: number, y: number) =>
  (y > r.start.y || (y === r.start.y && x >= r.start.x)) && (y < r.end.y || (y === r.end.y && x <= r.end.x));

/** The cell under a point of the window, as xterm counts cells (from 1), or null off the text. */
export function cellAt(term: Terminal, clientX: number, clientY: number): { x: number; y: number } | null {
  const screen = term.element?.querySelector('.xterm-screen');
  if (!screen || !term.cols || !term.rows) return null;
  const rect = screen.getBoundingClientRect();
  if (!rect.width || !rect.height) return null;
  const col = Math.floor((clientX - rect.left) / (rect.width / term.cols));
  const row = Math.floor((clientY - rect.top) / (rect.height / term.rows));
  if (col < 0 || col >= term.cols || row < 0 || row >= term.rows) return null;
  return { x: col + 1, y: term.buffer.active.viewportY + row + 1 };
}

/** The paths drawn under a point of the window, the longest first; empty when there is none. */
export function pathsAtPoint(term: Terminal, clientX: number, clientY: number): ScreenPath[] {
  const cell = cellAt(term, clientX, clientY);
  if (!cell) return [];
  return pathsOnRow(term, cell.y - 1).filter((p) => inRange(p.range, cell.x, cell.y));
}

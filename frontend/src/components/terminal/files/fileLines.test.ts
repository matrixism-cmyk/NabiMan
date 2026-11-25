import { Terminal } from 'xterm';
import { cellAt, pathsAtPoint, pathsOnRow } from './fileLines';

/** A real xterm (never opened) with `text` written to it. */
async function screen(text: string, cols = 40, rows = 10): Promise<Terminal> {
  const term = new Terminal({ cols, rows, allowProposedApi: true });
  await new Promise<void>((done) => term.write(text, done));
  return term;
}
const names = (term: Terminal, y: number) => pathsOnRow(term, y).map((p) => p.path);

describe('paths on a terminal screen', () => {
  it('places a path on its cells, counting a Korean letter as two', async () => {
    const term = await screen('at /tmp/강의.pdf ok');
    const [found] = pathsOnRow(term, 0);
    expect(found.path).toBe('/tmp/강의.pdf');
    // "at " is 3 cells; the path is 5 + 4 + 4 = 13 cells wide.
    expect(found.range).toEqual({ start: { x: 4, y: 1 }, end: { x: 16, y: 1 } });
  });

  it('reads a path the terminal wrapped as one, from either row', async () => {
    const path = '/workspace/5gmec/작업본/_전북진흥원_강의_20261203/deck/20261125_v3_V-Nexus2026_강의자료.pptx';
    const term = await screen(`see ${path} here`);
    expect(term.buffer.active.getLine(1)?.isWrapped).toBe(true);
    for (const y of [0, 1, 2]) expect(names(term, y)).toEqual([path]);
    const [found] = pathsOnRow(term, 1);
    expect(found.range.start).toEqual({ x: 5, y: 1 });
    expect(found.range.end.y).toBe(3);
    expect(names(term, 3)).toEqual([]);
  });

  it('reads on past a wide letter that did not fit at the edge', async () => {
    // 39 cells, then a two-cell letter: xterm leaves the last cell empty and wraps.
    const path = `/${'a'.repeat(33)}/dir/가나.txt`;
    const term = await screen(path);
    expect(term.buffer.active.getLine(1)?.isWrapped).toBe(true);
    expect(names(term, 0)).toEqual([path]);
  });

  it('joins a row a program filled to the edge with the next one, and keeps the plain reading', async () => {
    // Claude Code breaks a long word itself: a line end, then an indent.
    const head = '/workspace/proj/some/deep/folder/with/a/'; // 40 cells
    const term = await screen(`  ${head}\r\n  long-name.md and more`, 42);
    expect(term.buffer.active.getLine(1)?.isWrapped).toBe(false);
    expect(names(term, 0)).toEqual([`${head}long-name.md`, head]);
    expect(names(term, 1)).toEqual([`${head}long-name.md`, 'long-name.md']);
    expect(pathsOnRow(term, 1)[0].range).toEqual({ start: { x: 3, y: 1 }, end: { x: 14, y: 2 } });
  });

  it('does not join rows that end short of the edge', async () => {
    const term = await screen('see /tmp/a.txt\r\nb.txt');
    expect(names(term, 0)).toEqual(['/tmp/a.txt']);
    expect(names(term, 1)).toEqual(['b.txt']);
  });

  it('finds the cell and the path under a point', async () => {
    const term = await screen('at /tmp/a.txt ok\r\nplain words');
    const element = document.createElement('div');
    const grid = document.createElement('div');
    grid.className = 'xterm-screen';
    grid.getBoundingClientRect = () => ({ left: 100, top: 50, width: 400, height: 200, right: 500, bottom: 250, x: 100, y: 50, toJSON: () => ({}) });
    element.appendChild(grid);
    Object.defineProperty(term, 'element', { value: element });
    // Cells are 10 × 20 px here.
    expect(cellAt(term, 100, 50)).toEqual({ x: 1, y: 1 });
    expect(cellAt(term, 99, 50)).toBeNull();
    expect(cellAt(term, 135, 75)).toEqual({ x: 4, y: 2 });
    expect(pathsAtPoint(term, 135, 55).map((p) => p.path)).toEqual(['/tmp/a.txt']);
    expect(pathsAtPoint(term, 115, 55)).toEqual([]);
    expect(pathsAtPoint(term, 135, 75)).toEqual([]);
  });
});

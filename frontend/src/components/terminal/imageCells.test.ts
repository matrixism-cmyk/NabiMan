/**
 * Images drawn by tmux and full-screen programs must fill the cells they cover
 * even when a cell is a fractional number of CSS pixels.
 */
import { alignImagesToCells, reportedCell, scaledWidth } from './imageCells';

function setup(cell = { width: 8.43, height: 20.7 }) {
  let sizes: (params: (number | number[])[]) => boolean = () => false;
  const sent: string[] = [];
  const seen: unknown[] = [];
  const clears: number[][] = [];
  // like the addon's ImageRenderer: cellSize is a getter on the class
  class Renderer {
    dimensions = { css: { cell, canvas: { width: cell.width * 80, height: cell.height * 24 } } };
    _ctx = { clearRect: (...a: number[]) => clears.push(a) };
    clearLines(_start: number, _end: number) {}
    get cellSize() { return this.dimensions.css.cell; }
  }
  const renderer = new Renderer();
  const storage = { addImage: () => { seen.push(renderer.cellSize); } };
  const term = {
    cols: 80, rows: 24,
    parser: { registerCsiHandler: (_: unknown, h: typeof sizes) => { sizes = h; return { dispose: () => {} }; } },
    _core: { _renderService: { dimensions: renderer.dimensions }, coreService: { triggerDataEvent: (d: string) => sent.push(d) } },
  };
  const addon = { _renderer: renderer, _storage: storage };
  alignImagesToCells(term as never, addon as never);
  return { sizes, sent, seen, clears, renderer, storage };
}

test('both size reports give the same whole-pixel cell', () => {
  const { sizes, sent } = setup();
  expect(sizes([16])).toBe(true);
  expect(sizes([14])).toBe(true);
  expect(sent).toEqual(['\x1b[6;21;8t', `\x1b[4;${21 * 24};${8 * 80}t`]);
  expect(sizes([18])).toBe(false); // left to xterm
});

test('images are recorded at the reported cell size, so the addon stretches them onto the real cells', () => {
  const { storage, seen, renderer } = setup();
  storage.addImage();
  expect(seen).toEqual([{ width: 8, height: 21 }]);
  expect(renderer.cellSize).toEqual({ width: 8.43, height: 20.7 }); // drawing still uses the real cells
});

test('clearing rows takes exactly their pixel rows', () => {
  const { renderer, clears } = setup();
  renderer.clearLines(10, 11);
  // rows 10..11 are drawn from floor(10 * 20.7) = 207 up to floor(12 * 20.7) = 248
  expect(clears).toEqual([[0, 207, 8.43 * 80, 41]]);
});

test('no size before the first render', () => {
  expect(reportedCell(undefined)).toBeNull();
  expect(reportedCell({ width: 0, height: 0 })).toBeNull();
});

test('a rescaled image keeps the column count its tiles were numbered with', () => {
  // every whole-pixel cell the terminal may report, every real cell around it, images up to 200 cells
  for (let from = 5; from <= 24; from++) {
    for (let to = from - 0.99; to < from + 1; to += 0.01) {
      for (const cols of [1, 7, 20, 81, 150, 200]) {
        const w = scaledWidth(cols * from, from, to);
        expect([cols, from, to.toFixed(2), Math.ceil(w / to)]).toEqual([cols, from, to.toFixed(2), cols]);
        expect(w).toBeGreaterThanOrEqual(cols * to - 1 - 1e-9); // and at most a pixel short
      }
    }
  }
});

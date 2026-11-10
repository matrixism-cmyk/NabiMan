import type { IDisposable, Terminal } from 'xterm';
import type { ImageAddon } from 'xterm-addon-image';

/**
 * Pixel-exact images on cells that are not a whole number of pixels.
 *
 * xterm.js lays cells out at fractional CSS sizes (lineHeight 1.15, a device
 * pixel ratio of 1.25 or 2, the measured glyph width), but tells programs a
 * rounded size: CSI 16 t answers `toFixed(0)`, and CSI 14 t a rounded window
 * size that tmux divides by the cell count. Programs and tmux therefore draw
 * images at a whole-pixel cell size, while the image addon pastes them 1:1 onto
 * the fractional cells. Every image then falls a little short of the cells it
 * covers, and older images show through the gap: menus and dialogs drawn as
 * images leave ghosts and stripes behind.
 *
 * The fix keeps one whole-pixel cell size everywhere:
 * - both size reports give the same rounded cell (and a window of exactly
 *   cols x rows of them), so tmux and the program agree on it;
 * - each image is recorded as drawn at that size, so the addon's own rescaling
 *   stretches it onto the real cells;
 * - a row clears exactly the pixel rows it is drawn on, instead of a fractional
 *   band that takes half a pixel off the row below;
 * - a rescaled image keeps the tile columns it was numbered with (scaledWidth).
 *
 * It reaches into the addon (xterm-addon-image 0.5, the last release for the
 * `xterm` 5.3 package); anything missing leaves the addon's behaviour alone.
 */
export interface ReportedCell { width: number; height: number }

type Cell = { width: number; height: number };
interface ImageSpec {
  orig: HTMLCanvasElement | ImageBitmap;
  actual: HTMLCanvasElement | ImageBitmap;
  origCellSize: Cell;
  actualCellSize: Cell;
}
interface RendererInternals {
  dimensions?: { css: { cell: Cell; canvas: Cell } };
  document?: Document;
  _ctx?: CanvasRenderingContext2D;
  clearLines?: (start: number, end: number) => void;
  _rescaleImage?: (spec: ImageSpec, width: number, height: number) => void;
  cellSize?: Cell;
}
interface AddonInternals {
  _renderer?: RendererInternals;
  _storage?: { addImage?: (img: unknown) => void };
}
interface CoreInternals {
  _renderService?: { dimensions?: { css: { cell: Cell } } };
  coreService?: { triggerDataEvent: (data: string, wasUserInput?: boolean) => void };
}

/**
 * Width of an image of `width` px, drawn at cells `from` px wide, rescaled to
 * cells `to` px wide. The addon numbers an image's tiles with the column count
 * at the size it was drawn (`ceil(width / from)`) but finds them again with
 * `ceil(scaled / to)`; rounding the scaled width up (`156.0000000003` -> 157)
 * can make that one column more, and every row then starts one tile later: the
 * image comes out as a staircase with holes. Stay at or under `cols * to`.
 */
export function scaledWidth(width: number, from: number, to: number): number {
  const cols = Math.ceil(width / from);
  const exact = (width * to) / from;
  let w = Math.max(1, Math.min(Math.ceil(exact - 1e-6), Math.floor(cols * to)));
  // check with the addon's own expression, so float rounding cannot disagree with it
  while (w > 1 && Math.ceil(w / to) > cols) w--;
  return w;
}

/** The whole-pixel cell size told to programs, or null before the first render. */
export function reportedCell(cell: Cell | undefined): ReportedCell | null {
  if (!cell || !(cell.width > 0) || !(cell.height > 0)) return null;
  return { width: Math.max(1, Math.round(cell.width)), height: Math.max(1, Math.round(cell.height)) };
}

export function alignImagesToCells(term: Terminal, addon: ImageAddon): IDisposable {
  const core = (term as unknown as { _core?: CoreInternals })._core;
  const internals = addon as unknown as AddonInternals;
  const renderer = internals._renderer;
  const storage = internals._storage;
  const current = () => reportedCell(core?._renderService?.dimensions?.css.cell);

  const sizes = term.parser.registerCsiHandler({ final: 't' }, (params) => {
    const r = current();
    if (!r || !core?.coreService) return false;
    if (params[0] === 16) {
      core.coreService.triggerDataEvent(`\x1b[6;${r.height};${r.width}t`);
      return true;
    }
    if (params[0] === 14) {
      core.coreService.triggerDataEvent(`\x1b[4;${r.height * term.rows};${r.width * term.cols}t`);
      return true;
    }
    return false;
  });

  const originalClear = renderer?.clearLines;
  if (renderer && originalClear) {
    renderer.clearLines = (start: number, end: number) => {
      const ctx = renderer._ctx;
      const d = renderer.dimensions;
      if (!ctx || !d) return;
      const h = d.css.cell.height;
      const top = Math.floor(start * h);
      ctx.clearRect(0, top, d.css.canvas.width, Math.floor((end + 1) * h) - top);
    };
  }

  // The addon's own rescale, but with a width that keeps the tile columns (see scaledWidth).
  const originalRescale = renderer?._rescaleImage;
  if (renderer && originalRescale) {
    renderer._rescaleImage = (spec: ImageSpec, width: number, height: number) => {
      const done = spec.actualCellSize;
      if (width === done.width && height === done.height) return;
      const from = spec.origCellSize;
      const doc = renderer.document ?? document;
      if (width === from.width && height === from.height) {
        spec.actual = spec.orig;
      } else {
        const canvas = doc.createElement('canvas');
        canvas.width = scaledWidth(spec.orig.width, from.width, width);
        canvas.height = Math.max(1, Math.ceil((spec.orig.height * height) / from.height - 1e-6));
        const ctx = canvas.getContext('2d');
        if (!ctx) return;
        ctx.drawImage(spec.orig, 0, 0, canvas.width, canvas.height);
        spec.actual = canvas;
      }
      spec.actualCellSize = { width, height };
    };
  }

  const originalAdd = storage?.addImage;
  if (renderer && storage && originalAdd) {
    storage.addImage = (img: unknown) => {
      const r = current();
      if (!r) return originalAdd.call(storage, img);
      // addImage reads cellSize for both the cells an image covers and the size
      // it was drawn at; later draws rescale it from there to the real cells.
      Object.defineProperty(renderer, 'cellSize', { value: r, configurable: true });
      try {
        return originalAdd.call(storage, img);
      } finally {
        delete renderer.cellSize;
      }
    };
  }

  return {
    dispose: () => {
      sizes.dispose();
      if (renderer && originalClear) renderer.clearLines = originalClear;
      if (renderer && originalRescale) renderer._rescaleImage = originalRescale;
      if (storage && originalAdd) storage.addImage = originalAdd;
    },
  };
}

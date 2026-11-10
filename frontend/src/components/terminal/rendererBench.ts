import { Terminal } from 'xterm';
import { attachRenderer, Renderer, rendererState } from './renderers';

/**
 * How fast each renderer draws on this browser.
 *
 * A nearly transparent terminal (so the browser still renders it) gets a full
 * screen of colours, Korean and box lines, a new one each time the previous one
 * has been drawn. Two numbers:
 * - fps: screens per second the browser really shows, paint included; 60 is
 *   the monitor's own limit, so anything near it is fast enough.
 * - cpuMs: the renderer's own drawing call per screen (the DOM renderer's
 *   layout included). Paint and GPU work come later, so this leaves out
 *   different shares for each renderer — a hint, not the comparison.
 */
export interface BenchResult {
  requested: Renderer;
  /** What it really ran on (WebGL may fall back to Canvas). */
  active: Renderer | null;
  fps: number | null;
  cpuMs: number | null;
}

interface Internals { _core?: { _renderService?: { _renderRows?: (start: number, end: number) => void } } }

const COLS = 100;
const ROWS = 30;

function screen(frame: number): string {
  // i18n-exempt: benchmark text that must include wide Hangul cells
  const words = ['nabiman', '나비맨', '터미널', 'render', '─┼─', '│', '한글', 'WebGL', 'canvas', '0123456789'];
  let out = '\x1b[H';
  for (let r = 0; r < ROWS; r++) {
    let line = '';
    for (let i = 0; line.length < COLS - 12; i++) {
      const w = words[(r * 7 + i * 3 + frame) % words.length];
      line += `\x1b[38;5;${(r * 11 + i * 17 + frame) % 230 + 16}m${w} `;
    }
    out += `${line}\x1b[0m\x1b[K${r < ROWS - 1 ? '\r\n' : ''}`;
  }
  return out;
}

const write = (term: Terminal, data: string) => new Promise<void>((done) => term.write(data, done));

/** Next time the terminal draws (with a time limit: a hidden tab draws nothing). */
const nextRender = (term: Terminal) => new Promise<void>((done) => {
  const timer = window.setTimeout(() => { sub.dispose(); done(); }, 1000);
  const sub = term.onRender(() => { window.clearTimeout(timer); sub.dispose(); done(); });
});

async function measure(requested: Renderer, fontFamily: string, fontSize: number, frames: number): Promise<BenchResult> {
  const host = document.createElement('div');
  host.style.cssText = 'position:fixed;left:0;top:0;width:1100px;height:700px;opacity:0.02;pointer-events:none;z-index:-1';
  document.body.appendChild(host);
  const term = new Terminal({ cols: COLS, rows: ROWS, fontFamily, fontSize, lineHeight: 1.15, allowProposedApi: true });
  try {
    term.open(host);
    const renderer = attachRenderer(term, requested);
    const active = rendererState(term)?.active ?? null;
    const draw = (term as unknown as Internals)._core?._renderService?._renderRows?.bind((term as unknown as Internals)._core?._renderService);
    await write(term, screen(0));
    await nextRender(term); // 글자 모양을 처음 그려 두는 시간은 빼고
    // 화면이 실제로 바뀌는 속도: 한 장이 그려지면 바로 다음 장
    const t0 = performance.now();
    for (let f = 1; f <= frames; f++) {
      await write(term, screen(f));
      await nextRender(term);
    }
    const fps = (frames * 1000) / (performance.now() - t0);
    let cpu = 0;
    if (draw) {
      for (let f = 1; f <= 20; f++) {
        await write(term, screen(frames + f));
        const c0 = performance.now();
        draw(0, ROWS - 1);
        void host.offsetHeight; // DOM: 배치(layout)까지
        cpu += performance.now() - c0;
      }
    }
    renderer.dispose();
    return { requested, active, fps, cpuMs: draw ? cpu / 20 : null };
  } catch {
    return { requested, active: null, fps: null, cpuMs: null };
  } finally {
    term.dispose();
    host.remove();
  }
}

export async function benchmarkRenderers(fontFamily: string, fontSize: number, frames = 60): Promise<BenchResult[]> {
  const out: BenchResult[] = [];
  for (const r of ['webgl', 'canvas', 'dom'] as Renderer[]) {
    out.push(await measure(r, fontFamily, fontSize, frames));
  }
  return out;
}

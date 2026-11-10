/**
 * Which renderer a terminal ends up with: WebGL, then Canvas, then DOM, and a
 * window whose WebGL context is lost carries on with Canvas.
 */
import { attachRenderer, readRendererChoice, rendererStates, writeRendererChoice, RendererAddons } from './renderers';

jest.mock('xterm-addon-webgl', () => ({ WebglAddon: jest.fn() }));
jest.mock('xterm-addon-canvas', () => ({ CanvasAddon: jest.fn() }));

function fake(opts: { webgl?: 'ok' | 'throws'; canvas?: 'ok' | 'throws'; software?: boolean } = {}) {
  const loaded: string[] = [];
  const disposed: string[] = [];
  let lose: () => void = () => {};
  const addon = (name: string, mode: 'ok' | 'throws' = 'ok') => ({
    activate: () => { if (mode === 'throws') throw new Error(`${name} unavailable`); },
    dispose: () => disposed.push(name),
    onContextLoss: (cb: () => void) => { lose = cb; return { dispose: () => {} }; },
    name,
    mode,
  });
  const term = {
    loadAddon: (a: ReturnType<typeof addon>) => { a.activate(); loaded.push(a.name); },
  };
  const addons: RendererAddons = {
    webgl: () => addon('webgl', opts.webgl),
    canvas: () => addon('canvas', opts.canvas),
    softwareWebgl: () => opts.software ?? false,
  };
  return { term: term as never, addons, loaded, disposed, lose: () => lose() };
}

const stateOf = () => rendererStates()[rendererStates().length - 1];

test('auto takes WebGL when the GPU does it', () => {
  const f = fake();
  const r = attachRenderer(f.term, 'auto', f.addons);
  expect(f.loaded).toEqual(['webgl']);
  expect(stateOf()).toEqual({ active: 'webgl' });
  r.dispose();
  expect(f.disposed).toEqual(['webgl']);
});

test('software WebGL, failing WebGL and failing Canvas each fall back one step', () => {
  let f = fake({ software: true });
  let r = attachRenderer(f.term, 'auto', f.addons);
  expect([f.loaded, stateOf()]).toEqual([['canvas'], { active: 'canvas', reason: 'software-webgl' }]);
  r.dispose();
  f = fake({ webgl: 'throws' });
  r = attachRenderer(f.term, 'auto', f.addons);
  expect(stateOf()).toEqual({ active: 'canvas', reason: 'webgl-failed' });
  r.dispose();
  f = fake({ webgl: 'throws', canvas: 'throws' });
  r = attachRenderer(f.term, 'auto', f.addons);
  expect(stateOf()).toEqual({ active: 'dom', reason: 'canvas-failed' });
  r.dispose();
});

test('a lost WebGL context moves that window to Canvas', () => {
  const f = fake();
  const r = attachRenderer(f.term, 'webgl', f.addons);
  f.lose();
  expect(f.loaded).toEqual(['webgl', 'canvas']);
  expect(f.disposed).toEqual(['webgl']);
  expect(stateOf()).toEqual({ active: 'canvas', reason: 'webgl-lost' });
  r.dispose();
  expect(rendererStates()).toEqual([]);
});

test('an explicit choice skips the software check; the choice is kept per browser', () => {
  const f = fake({ software: true });
  attachRenderer(f.term, 'webgl', f.addons).dispose();
  expect(f.loaded).toEqual(['webgl']);
  const d = fake();
  attachRenderer(d.term, 'dom', d.addons).dispose();
  expect(d.loaded).toEqual([]);
  expect(readRendererChoice()).toBe('auto');
  writeRendererChoice('canvas');
  expect(readRendererChoice()).toBe('canvas');
  writeRendererChoice('auto');
  expect(localStorage.getItem('nabiman.terminalRenderer')).toBeNull();
});

import { watchContext } from './glContext';

function fakeCanvas() {
  const on = new Map<string, (e: { preventDefault(): void }) => void>();
  return {
    on,
    addEventListener: (t: string, fn: (e: { preventDefault(): void }) => void) => { on.set(t, fn); },
    removeEventListener: (t: string) => { on.delete(t); },
  };
}

test('a lost context is let come back, and the scene redrawn when it does', () => {
  const c = fakeCanvas();
  const redraw = jest.fn();
  const stop = watchContext(c, redraw);
  const e = { preventDefault: jest.fn() };
  c.on.get('webglcontextlost')!(e);
  expect(e.preventDefault).toHaveBeenCalled();
  expect(redraw).not.toHaveBeenCalled();
  c.on.get('webglcontextrestored')!({ preventDefault: jest.fn() });
  expect(redraw).toHaveBeenCalledTimes(1);
  expect(stop.counts).toEqual({ lost: 1, restored: 1 });
  stop();
  expect(c.on.size).toBe(0);
});

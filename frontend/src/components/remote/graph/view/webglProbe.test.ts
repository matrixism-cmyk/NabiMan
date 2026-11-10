import { hasWebGL, resetWebGLProbe } from './webglProbe';

function fakeDoc(withGl: boolean) {
  const lose = jest.fn();
  const getContext = jest.fn(() => (withGl ? { getExtension: (name: string) => (name === 'WEBGL_lose_context' ? { loseContext: lose } : null) } : null));
  const createElement = jest.fn(() => ({ getContext }));
  return { doc: { createElement }, createElement, getContext, lose };
}

beforeEach(() => resetWebGLProbe());

test('probes once, however often it is asked, and hands the context back', () => {
  const f = fakeDoc(true);
  for (let i = 0; i < 50; i += 1) expect(hasWebGL(f.doc)).toBe(true);
  expect(f.createElement).toHaveBeenCalledTimes(1);
  expect(f.lose).toHaveBeenCalledTimes(1);
});

test('no WebGL: says so, and remembers it', () => {
  const f = fakeDoc(false);
  expect(hasWebGL(f.doc)).toBe(false);
  expect(hasWebGL(f.doc)).toBe(false);
  expect(f.createElement).toHaveBeenCalledTimes(1);
  expect(f.getContext).toHaveBeenCalledWith('webgl');
});

test('a throwing canvas counts as no WebGL', () => {
  const doc = { createElement: () => { throw new Error('blocked'); } };
  expect(hasWebGL(doc as never)).toBe(false);
});

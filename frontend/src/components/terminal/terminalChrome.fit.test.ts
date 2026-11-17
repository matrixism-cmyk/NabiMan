import { fitFontToPane, mirrorGrid } from './terminalChrome';

const term = (fontSize = 14) => ({ options: { fontSize, lineHeight: 1 }, cols: 80, rows: 24, resize: jest.fn() }) as any;

test('a terminal with no cell size yet keeps its text and does not throw', () => {
  const t = term();
  const fit = { proposeDimensions: () => { throw new TypeError("Cannot read properties of undefined (reading 'dimensions')"); } } as any;
  expect(() => fitFontToPane(t, fit, 120, 40)).not.toThrow();
  expect(t.options.fontSize).toBe(14);
  expect(() => mirrorGrid(t, fit, 120, 40)).not.toThrow();
});

test('a terminal the addon cannot measure keeps its text', () => {
  const t = term();
  fitFontToPane(t, { proposeDimensions: () => undefined } as any, 120, 40);
  expect(t.options.fontSize).toBe(14);
});

test('a measured terminal still gets its text scaled to show every column', () => {
  const t = term();
  // 1,000 px wide, a cell about 0.6 of the font size.
  const fit = { proposeDimensions: () => ({ cols: Math.floor(1000 / (t.options.fontSize * 0.6)), rows: 100 }) } as any;
  fitFontToPane(t, fit, 120, 40);
  expect(t.options.fontSize).toBeLessThan(14);
  expect(Math.floor(1000 / (t.options.fontSize * 0.6))).toBeGreaterThanOrEqual(120);
});

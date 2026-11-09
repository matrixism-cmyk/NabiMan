import { Box, rounded, route } from './SessionLinks';

// Three columns of 100×80 cards with 12 px gaps, three rows: like the sessions grid.
const at = (col: number, row: number): Box => ({ l: col * 112, t: row * 92, r: col * 112 + 100, b: row * 92 + 80 });
const cards = [0, 1, 2].flatMap((row) => [0, 1, 2].map((col) => at(col, row)));
const card = (col: number, row: number) => cards[row * 3 + col];
/** Whether a segment runs through a card other than its two ends. */
const crosses = (pts: [number, number][], ends: Box[]) => pts.slice(1).some(([x2, y2], i) => {
  const [x1, y1] = pts[i];
  return cards.some((k) => !ends.includes(k) && Math.max(x1, x2) > k.l && Math.min(x1, x2) < k.r && Math.max(y1, y2) > k.t && Math.min(y1, y2) < k.b);
});

it('goes straight to a neighbour, side to side or top to bottom', () => {
  expect(route(card(0, 0), card(1, 0), cards, 12, 12)).toEqual([[100, 40], [109, 40]]);
  expect(route(card(1, 1), card(1, 0), cards, 12, 12)).toEqual([[50 + 112, 92], [50 + 112, 83]]);
});

it('never crosses another card: two rows down and one column over goes through the gaps', () => {
  // The case that looked wrong: 5gmec (middle, top) to a card two rows below on the right.
  const pts = route(card(1, 0), card(2, 2), cards, 12, 12);
  expect(crosses(pts, [card(1, 0), card(2, 2)])).toBe(false);
  expect(pts[0]).toEqual([162, 80]);           // leaves the source's bottom
  expect(pts[pts.length - 1]).toEqual([274, 181]); // and points down into the target's top
  // Across the row gap under the source, down the column gap left of the target.
  expect(pts.map(([x]) => x)).toContain(218);
});

it('finds a way round the cards in between, in a row, a column or across', () => {
  for (const [a, b] of [[card(0, 0), card(2, 0)], [card(0, 0), card(0, 2)], [card(2, 2), card(0, 0)], [card(0, 1), card(2, 0)], [card(2, 0), card(2, 2)]]) {
    expect(crosses(route(a, b, cards, 12, 12), [a, b])).toBe(false);
  }
});

it('arrows that share a gap are drawn apart', () => {
  const one = route(card(0, 0), card(1, 1), cards, 12, 12, 0);
  const two = route(card(0, 0), card(1, 1), cards, 12, 12, 4);
  expect(one[1][1]).not.toBe(two[1][1]);
});

it('rounds the corners', () => {
  expect(rounded([[0, 0], [0, 20], [30, 20]])).toBe('M0.0,0.0 L0.0,12.0 Q0.0,20.0 8.0,20.0 L30.0,20.0');
});

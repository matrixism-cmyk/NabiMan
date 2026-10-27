import { needsShift, typeText } from './vncInput';

jest.mock('@novnc/novnc/lib/rfb.js', () => ({}));

const fake = () => {
  const log: string[] = [];
  return { log, rfb: { sendKey: (k: number, _c: null, down = true) => log.push(`${k.toString(16)}${down ? '↓' : '↑'}`) } as any };
};

test('capitals and shifted symbols are typed with Shift held, the rest without', () => {
  expect(['A', 'Z', '!', '@', '?', '_'].map((c) => needsShift(c.charCodeAt(0)))).toEqual([true, true, true, true, true, true]);
  expect(['a', '1', '-', '/', ' '].map((c) => needsShift(c.charCodeAt(0)))).toEqual([false, false, false, false, false]);
});

test('a password with a capital and a ! arrives as typed', () => {
  const { log, rfb } = fake();
  typeText(rfb, 'aB1!\n');
  expect(log).toEqual(['61↓', '61↑', 'ffe1↓', '42↓', '42↑', 'ffe1↑', '31↓', '31↑', 'ffe1↓', '21↓', '21↑', 'ffe1↑', 'ff0d↓', 'ff0d↑']);
});

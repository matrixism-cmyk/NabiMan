import { applyArmed, armModifiers, keySequence, withMods } from './stickyKeys';

test('arrows follow the cursor mode, and carry modifiers the xterm way', () => {
  expect(keySequence('ArrowUp', false)).toBe('\x1b[A');
  expect(keySequence('ArrowUp', true)).toBe('\x1bOA');
  expect(keySequence('ArrowLeft', true, { ctrl: true, alt: false })).toBe('\x1b[1;5D');
  expect(keySequence('End', false, { ctrl: false, alt: true })).toBe('\x1b[1;3F');
  expect(keySequence('PageDown', false)).toBe('\x1b[6~');
  expect(keySequence('Escape', false)).toBe('\x1b');
  expect(keySequence('Tab', false, { ctrl: false, alt: true })).toBe('\x1b\t');
});

test('Ctrl and Alt change what is typed next', () => {
  expect(withMods('c', { ctrl: true, alt: false })).toBe('\x03');
  expect(withMods('D', { ctrl: true, alt: false })).toBe('\x04');
  expect(withMods('[', { ctrl: true, alt: false })).toBe('\x1b');
  expect(withMods('b', { ctrl: false, alt: true })).toBe('\x1bb');
  expect(withMods('x', { ctrl: true, alt: true })).toBe('\x1b\x18');
  expect(withMods('한', { ctrl: true, alt: false })).toBe('한');
});

test('armed modifiers apply once, then let go', () => {
  const term = {};
  const used = jest.fn();
  armModifiers(term, { ctrl: true, alt: false }, used);
  expect(applyArmed(term, 'c')).toBe('\x03');
  expect(used).toHaveBeenCalledTimes(1);
  expect(applyArmed(term, 'c')).toBe('c');
  armModifiers(term, { ctrl: true, alt: false }, used);
  armModifiers(term, { ctrl: false, alt: false }, used);
  expect(applyArmed(term, 'c')).toBe('c');
});

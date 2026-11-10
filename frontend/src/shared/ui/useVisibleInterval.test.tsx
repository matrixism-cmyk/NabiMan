import { act, renderHook } from '@testing-library/react';
import { startVisibleInterval, useVisibleInterval } from './useVisibleInterval';

let hidden = false;
beforeAll(() => {
  Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden });
});
afterAll(() => { delete (document as unknown as { hidden?: boolean }).hidden; });
beforeEach(() => { hidden = false; jest.useFakeTimers(); });
afterEach(() => jest.useRealTimers());

const setHidden = (value: boolean) => {
  hidden = value;
  document.dispatchEvent(new Event('visibilitychange'));
};

test('polls while visible, rests while hidden, and catches up at once when shown', () => {
  const tick = jest.fn();
  renderHook(() => useVisibleInterval(tick, 3000));
  act(() => { jest.advanceTimersByTime(9000); });
  expect(tick).toHaveBeenCalledTimes(3);

  act(() => setHidden(true));
  act(() => { jest.advanceTimersByTime(60_000); });
  expect(tick).toHaveBeenCalledTimes(3);

  act(() => setHidden(false));
  expect(tick).toHaveBeenCalledTimes(4);
  act(() => { jest.advanceTimersByTime(3000); });
  expect(tick).toHaveBeenCalledTimes(5);
});

test('a tab opened hidden waits until it is shown', () => {
  hidden = true;
  const tick = jest.fn();
  const stop = startVisibleInterval(tick, 1000);
  jest.advanceTimersByTime(5000);
  expect(tick).not.toHaveBeenCalled();
  setHidden(false);
  expect(tick).toHaveBeenCalledTimes(1);
  stop();
  setHidden(true); setHidden(false);
  jest.advanceTimersByTime(5000);
  expect(tick).toHaveBeenCalledTimes(1);
});

test('no interval means no polling, and the latest callback is the one called', () => {
  const first = jest.fn();
  const second = jest.fn();
  const { rerender } = renderHook(({ fn, ms }) => useVisibleInterval(fn, ms), { initialProps: { fn: first, ms: 0 as number } });
  act(() => { jest.advanceTimersByTime(10_000); });
  expect(first).not.toHaveBeenCalled();
  rerender({ fn: second, ms: 1000 });
  act(() => { jest.advanceTimersByTime(1000); });
  expect(second).toHaveBeenCalledTimes(1);
  expect(first).not.toHaveBeenCalled();
});

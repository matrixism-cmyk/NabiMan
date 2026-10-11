import { act, renderHook } from '@testing-library/react';
import { useAuthCooldown } from './useAuthCooldown';
const response = (status: number, retry: string) => ({ status, headers: { get: () => retry } } as unknown as Response);
afterEach(() => jest.useRealTimers());
test('honors server delay and wall-clock expiry without retrying credentials', () => {
  jest.useFakeTimers(); const { result } = renderHook(useAuthCooldown);
  act(() => { expect(result.current.accept(response(200, '300'))).toBe(false); });
  expect(result.current.seconds).toBe(0);
  act(() => { expect(result.current.accept(response(429, '3'))).toBe(true); });
  expect(result.current.seconds).toBe(3);
  act(() => jest.advanceTimersByTime(2000)); expect(result.current.seconds).toBe(1);
  act(() => jest.advanceTimersByTime(1000)); expect(result.current.seconds).toBe(0);
});
test('invalid and excessive delays remain bounded', () => {
  jest.useFakeTimers(); const { result, unmount } = renderHook(useAuthCooldown);
  act(() => { result.current.accept(response(429, 'garbage')); }); expect(result.current.seconds).toBe(60);
  act(() => { result.current.accept(response(429, '999999')); }); expect(result.current.seconds).toBe(300);
  unmount(); expect(jest.getTimerCount()).toBe(0);
});

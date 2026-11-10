import { currentNotices, dismissNotice, installGlobalErrorHandlers, reportGlobalError, resetNoticesForTest } from './globalErrors';

beforeEach(() => resetNoticesForTest());

test('the same error repeating counts up on one notice instead of stacking', () => {
  reportGlobalError(new Error('poll failed'), 1000);
  reportGlobalError(new Error('poll failed'), 4000);
  reportGlobalError(new Error('poll failed'), 7000);
  expect(currentNotices()).toHaveLength(1);
  expect(currentNotices()[0].count).toBe(3);
});

test('a dismissed error stays quiet for a minute, then may show again', () => {
  reportGlobalError(new Error('boom'), 0);
  dismissNotice(currentNotices()[0].id);
  reportGlobalError(new Error('boom'), 30_000);
  expect(currentNotices()).toHaveLength(0);
  reportGlobalError(new Error('boom'), 61_000);
  expect(currentNotices()).toHaveLength(1);
});

test('a burst of different errors is capped, and at most three show', () => {
  for (let i = 0; i < 10; i += 1) reportGlobalError(new Error(`e${i}`), 1000 + i);
  expect(currentNotices().length).toBeLessThanOrEqual(3);
  currentNotices().forEach((n) => dismissNotice(n.id));
  reportGlobalError(new Error('e-late'), 2000);
  expect(currentNotices()).toHaveLength(0);
});

test('cancelled requests are not errors worth telling', () => {
  reportGlobalError(new DOMException('Login session changed; request discarded', 'AbortError'));
  reportGlobalError(new Error('ResizeObserver loop completed with undelivered notifications.'));
  expect(currentNotices()).toHaveLength(0);
});

test('unhandled rejections on the window reach the notices', () => {
  const uninstall = installGlobalErrorHandlers();
  const event = new Event('unhandledrejection') as PromiseRejectionEvent;
  Object.defineProperty(event, 'reason', { value: new Error('nobody caught me') });
  window.dispatchEvent(event);
  expect(currentNotices()[0].message).toContain('nobody caught me');
  uninstall();
});

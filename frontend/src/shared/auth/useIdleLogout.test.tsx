import React from 'react';
import { act, render } from '@testing-library/react';
import { apiRequest } from '../api/useApi';
import { markActivity, useIdleLogout } from './useIdleLogout';

jest.mock('../api/useApi', () => ({ apiRequest: jest.fn() }));
const request = apiRequest as jest.Mock;

let state: ReturnType<typeof useIdleLogout>;
function Probe({ active, signOut }: { active: boolean; signOut: () => void }) {
  state = useIdleLogout(active, signOut);
  return null;
}

const flush = async () => { await act(async () => { await Promise.resolve(); await Promise.resolve(); }); };
const pass = async (ms: number) => { await act(async () => { jest.advanceTimersByTime(ms); }); };

beforeEach(() => {
  jest.useFakeTimers();
  localStorage.clear();
  sessionStorage.clear();
  request.mockReset();
});
afterEach(() => jest.useRealTimers());

const withLimit = (minutes: number) => request.mockImplementation((path: string) =>
  Promise.resolve(path === '/api/app-settings' ? { success: true, data: { idle_logout_minutes: minutes } } : { success: true }));

test('off, it never signs out', async () => {
  withLimit(0);
  const signOut = jest.fn();
  render(<Probe active signOut={signOut} />);
  await flush();
  await pass(10 * 60_000);
  expect(signOut).not.toHaveBeenCalled();
});

test('it warns a minute before, and signs out when the time is up', async () => {
  withLimit(5);
  const signOut = jest.fn();
  render(<Probe active signOut={signOut} />);
  await flush();
  await pass(3 * 60_000);
  expect(state.secondsLeft).toBeNull();
  await pass(60_000 + 5_000);
  expect(state.secondsLeft).toBeGreaterThan(0);
  expect(state.secondsLeft).toBeLessThanOrEqual(60);
  await pass(60_000);
  expect(signOut).toHaveBeenCalled();
  expect(sessionStorage.getItem('nabiman_idle_logout')).toBe('1');
});

test('a key, a click or speech puts it off, and the server hears of it', async () => {
  withLimit(5);
  const signOut = jest.fn();
  render(<Probe active signOut={signOut} />);
  await flush();
  await pass(4 * 60_000);
  act(() => { window.dispatchEvent(new KeyboardEvent('keydown', { key: 'a' })); });
  await pass(4 * 60_000);
  act(() => { markActivity(); });
  await pass(4 * 60_000);
  expect(signOut).not.toHaveBeenCalled();
  expect(request.mock.calls.filter(([p]) => p === '/api/auth/activity').length).toBeGreaterThanOrEqual(2);
  await pass(2 * 60_000);
  expect(signOut).toHaveBeenCalled();
});

test('use in another tab counts here too', async () => {
  withLimit(5);
  const signOut = jest.fn();
  render(<Probe active signOut={signOut} />);
  await flush();
  await pass(4 * 60_000);
  localStorage.setItem('nabiman_last_activity', String(Date.now()));
  await pass(4 * 60_000);
  expect(signOut).not.toHaveBeenCalled();
});

import { act, renderHook } from '@testing-library/react';
import React from 'react';
import { I18nProvider } from '../../i18n';
import { LEAVE_MS, useSessionEnd } from './useSessionEnd';

const mockRequest = jest.fn();
const mockConfirm = jest.fn();
const mockNotice = jest.fn();
jest.mock('../../shared/api/useApi', () => ({ apiRequest: (...args: unknown[]) => mockRequest(...args) }));
jest.mock('../../shared/ui/ConfirmDialog', () => ({ confirmDialog: (o: unknown) => mockConfirm(o), noticeDialog: (m: string) => mockNotice(m) }));

const wrapper = ({ children }: { children: React.ReactNode }) => <I18nProvider>{children}</I18nProvider>;

function setup(over: Partial<Parameters<typeof useSessionEnd>[0]> = {}) {
  let connected = new Set(['ws_srv_1', 'ws_local']);
  const calls = { setStatus: jest.fn(), setJoinSessionId: jest.fn(), showSessions: jest.fn(), refetchSessions: jest.fn() };
  const setConnected = jest.fn((next: any) => { connected = typeof next === 'function' ? next(connected) : next; });
  const base = {
    visible: true, status: 'connected' as 'connected' | 'ended', storageKey: 'ws_srv_1', sessionId: 'pty_a',
    sessions: [{ id: 'pty_a', title: '배포 작업' }], setConnected, ...calls,
  };
  const hook = renderHook((props: Partial<typeof base>) => useSessionEnd({ ...base, ...over, ...props } as any), { wrapper, initialProps: {} });
  return { hook, calls, setConnected, connected: () => connected };
}

beforeEach(() => { localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); mockRequest.mockReset(); mockConfirm.mockReset(); mockNotice.mockReset(); });

test('ending asks first and ends the session; the pane lets go when the terminal reports it, or after a while if it never does', async () => {
  jest.useFakeTimers();
  mockConfirm.mockResolvedValue(true); mockRequest.mockResolvedValue({ success: true });
  const { hook, calls, connected } = setup();
  await act(async () => { await hook.result.current.end(); });
  expect(mockConfirm.mock.calls[0][0].message).toContain('배포 작업');
  expect(mockRequest).toHaveBeenCalledWith('/api/terminal/sessions/pty_a', { method: 'DELETE' });
  // Not yet: the terminal is still on screen.
  expect(calls.showSessions).not.toHaveBeenCalled();
  act(() => { jest.advanceTimersByTime(LEAVE_MS * 4 + 10); });
  expect(connected().has('ws_srv_1')).toBe(false);
  expect(connected().has('ws_local')).toBe(true);
  expect(calls.setJoinSessionId).toHaveBeenCalledWith(undefined);
  expect(calls.showSessions).toHaveBeenCalledTimes(1);
  expect(calls.setStatus).toHaveBeenCalledWith('connecting');
  jest.useRealTimers();
});

test('after ending, a pane that already moved on to another session is left alone', async () => {
  jest.useFakeTimers();
  mockConfirm.mockResolvedValue(true); mockRequest.mockResolvedValue({ success: true });
  const { hook, calls } = setup();
  await act(async () => { await hook.result.current.end(); });
  hook.rerender({ sessionId: 'pty_new' });
  act(() => { jest.advanceTimersByTime(LEAVE_MS * 4 + 10); });
  expect(calls.showSessions).not.toHaveBeenCalled();
  jest.useRealTimers();
});

test('saying no, or a failed end, leaves the pane as it is', async () => {
  mockConfirm.mockResolvedValue(false);
  const a = setup();
  await act(async () => { await a.hook.result.current.end(); });
  expect(mockRequest).not.toHaveBeenCalled();
  expect(a.calls.showSessions).not.toHaveBeenCalled();

  mockConfirm.mockResolvedValue(true); mockRequest.mockResolvedValue({ success: false, message: 'busy' });
  const b = setup();
  await act(async () => { await b.hook.result.current.end(); });
  expect(mockNotice.mock.calls[0][0]).toContain('busy');
  expect(b.calls.showSessions).not.toHaveBeenCalled();
  expect(b.connected().has('ws_srv_1')).toBe(true);
});

test('a session that ended by itself leaves the pane a moment later — only while the pane shows', () => {
  jest.useFakeTimers();
  const { hook, calls, connected } = setup();
  hook.rerender({ status: 'ended', visible: false });
  act(() => { jest.advanceTimersByTime(LEAVE_MS * 3); });
  expect(calls.showSessions).not.toHaveBeenCalled();
  hook.rerender({ status: 'ended', visible: true });
  // Its last lines stay for a moment, so the terminal is not torn down mid-draw.
  act(() => { jest.advanceTimersByTime(LEAVE_MS - 20); });
  expect(calls.showSessions).not.toHaveBeenCalled();
  act(() => { jest.advanceTimersByTime(40); });
  expect(calls.showSessions).toHaveBeenCalledTimes(1);
  expect(connected().has('ws_srv_1')).toBe(false);
  expect(calls.setStatus).toHaveBeenCalledWith('connecting');
  expect(calls.refetchSessions).toHaveBeenCalled();
  jest.useRealTimers();
});

test('a pane with no session yet has nothing to end', async () => {
  const { hook } = setup({ sessionId: '' });
  await act(async () => { await hook.result.current.end(); });
  expect(mockConfirm).not.toHaveBeenCalled();
});

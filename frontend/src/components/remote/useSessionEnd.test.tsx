import { act, renderHook } from '@testing-library/react';
import React from 'react';
import { I18nProvider } from '../../i18n';
import { useSessionEnd } from './useSessionEnd';

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

test('ending asks first, ends the session, then lets go of the pane and shows the session list', async () => {
  mockConfirm.mockResolvedValue(true); mockRequest.mockResolvedValue({ success: true });
  const { hook, calls, connected } = setup();
  await act(async () => { await hook.result.current.end(); });
  expect(mockConfirm.mock.calls[0][0].message).toContain('배포 작업');
  expect(mockRequest).toHaveBeenCalledWith('/api/terminal/sessions/pty_a', { method: 'DELETE' });
  expect(connected().has('ws_srv_1')).toBe(false);
  expect(connected().has('ws_local')).toBe(true);
  expect(calls.setJoinSessionId).toHaveBeenCalledWith(undefined);
  expect(calls.showSessions).toHaveBeenCalledTimes(1);
  expect(calls.refetchSessions).toHaveBeenCalledTimes(1);
  expect(calls.setStatus).toHaveBeenCalledWith('connecting');
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

test('a session that ended by itself leaves the pane at once — only while the pane shows', () => {
  const { hook, calls, connected } = setup();
  expect(calls.showSessions).not.toHaveBeenCalled();
  hook.rerender({ status: 'ended', visible: false });
  expect(calls.showSessions).not.toHaveBeenCalled();
  hook.rerender({ status: 'ended', visible: true });
  expect(calls.showSessions).toHaveBeenCalledTimes(1);
  expect(connected().has('ws_srv_1')).toBe(false);
  expect(calls.setStatus).toHaveBeenCalledWith('connecting');
  expect(calls.refetchSessions).toHaveBeenCalled();
});

test('a pane with no session yet has nothing to end', async () => {
  const { hook } = setup({ sessionId: '' });
  await act(async () => { await hook.result.current.end(); });
  expect(mockConfirm).not.toHaveBeenCalled();
});

import React from 'react';
import { act, fireEvent, render, renderHook, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { apiRequest } from '../../shared/api/useApi';
import FirstRunGuide from './FirstRunGuide';
import { looksFresh, readFirstRun, writeFirstRun } from './firstRunState';
import { takeStep, unavailableSteps, useFirstRun } from './useFirstRun';
import type { Tab } from '../../nav';

jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
const api = apiRequest as jest.Mock;

function answer(servers: unknown[], sessions: unknown[]) {
  api.mockImplementation((url: string) => Promise.resolve({ success: true, data: url === '/api/remote-servers' ? servers : sessions }));
}

beforeEach(() => { localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); });

describe('first-run state', () => {
  it('remembers dismissal and steps per account, and survives junk', () => {
    expect(readFirstRun('admin')).toEqual({ dismissed: false, done: [] });
    writeFirstRun('admin', { dismissed: true, done: ['server'] });
    expect(readFirstRun('admin')).toEqual({ dismissed: true, done: ['server'] });
    expect(readFirstRun('other')).toEqual({ dismissed: false, done: [] });
    localStorage.setItem('nabiman_first_run:junk', '{"done":["nope","backup"]');
    expect(readFirstRun('junk')).toEqual({ dismissed: false, done: [] });
    localStorage.setItem('nabiman_first_run:odd', '{"done":["nope","backup"]}');
    expect(readFirstRun('odd').done).toEqual(['backup']);
  });

  it('calls an install fresh only when both lists are readable and empty', async () => {
    answer([], []);
    await expect(looksFresh()).resolves.toBe(true);
    answer([{ id: 1 }], []);
    await expect(looksFresh()).resolves.toBe(false);
    api.mockResolvedValue({ success: false, message: 'Forbidden' });
    await expect(looksFresh()).resolves.toBe(false);
    api.mockRejectedValue(new Error('offline'));
    await expect(looksFresh()).resolves.toBe(false);
  });

  it('maps steps to screens and drops the ones this account lacks', () => {
    const navigate = jest.fn(), changePassword = jest.fn();
    takeStep('password', { navigate, changePassword });
    expect(changePassword).toHaveBeenCalled();
    takeStep('server', { navigate, changePassword });
    expect(navigate).toHaveBeenLastCalledWith('remote', 'remote');
    expect(JSON.parse(localStorage.getItem('nabiman_remote_selection') || '{}')).toEqual({ kind: 'new' });
    takeStep('backup', { navigate, changePassword });
    expect(navigate).toHaveBeenLastCalledWith('management', 'backup');
    takeStep('users', { navigate, changePassword });
    expect(navigate).toHaveBeenLastCalledWith('administration', 'usermgmt');
    expect(unavailableSteps(new Set<Tab>(['backup']))).toEqual(['backup']);
  });
});

describe('useFirstRun', () => {
  it('opens by itself on a fresh install until dismissed', async () => {
    answer([], []);
    const { result, unmount } = renderHook(() => useFirstRun('admin', true));
    await waitFor(() => expect(result.current.open).toBe(true));
    act(() => result.current.markDone('server'));
    act(() => result.current.dismiss());
    expect(result.current.open).toBe(false);
    expect(readFirstRun('admin')).toEqual({ dismissed: true, done: ['server'] });
    unmount();
    // Dismissed: not again by itself, but the header link still shows it.
    const again = renderHook(() => useFirstRun('admin', true));
    await act(async () => { await Promise.resolve(); });
    expect(again.result.current.open).toBe(false);
    act(() => again.result.current.show());
    expect(again.result.current.open).toBe(true);
  });

  it('stays shut on a used install and for non-administrators', async () => {
    answer([{ id: 1 }], []);
    const used = renderHook(() => useFirstRun('admin', true));
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(used.result.current.open).toBe(false);
    answer([], []);
    const viewer = renderHook(() => useFirstRun('viewer', false));
    act(() => viewer.result.current.show());
    expect(viewer.result.current.open).toBe(false);
  });
});

describe('FirstRunGuide', () => {
  it('lists the steps, takes one, reads the guide and dismisses', async () => {
    api.mockResolvedValue({ success: true, data: { text: '# 오케스트레이션 안내' } });
    const onStep = jest.fn(), onDismiss = jest.fn();
    render(<I18nProvider><FirstRunGuide done={['password']} unavailable={['users']} onStep={onStep} onDismiss={onDismiss} /></I18nProvider>);
    expect(screen.getByRole('complementary', { name: '처음 시작하기' })).toBeInTheDocument();
    expect(screen.getByText(/1\/5 완료/)).toBeInTheDocument();
    expect(screen.queryByText('사용자 초대하기')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /서버 추가하기/ }));
    expect(onStep).toHaveBeenCalledWith('server');
    fireEvent.click(screen.getByRole('button', { name: /오케스트레이션 안내 읽기/ }));
    expect(await screen.findByText('# 오케스트레이션 안내')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '닫기' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(onDismiss).not.toHaveBeenCalled();
    fireEvent.click(screen.getAllByRole('button', { name: '다시 보지 않기' })[1]);
    expect(onDismiss).toHaveBeenCalled();
  });
});

import React, { createRef } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../i18n';
import { apiRequest } from '../../shared/api/useApi';
import { MemberContext } from '../remote/member';
import AiGuideButton from './AiGuideButton';
import { bracketedPaste, forgetGuides } from './aiGuide';
import type { TerminalViewHandle } from './terminalTypes';

jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
const request = apiRequest as jest.Mock;

const GUIDE = '> 이 안내는 NabiMan 세션 «build» (pty_1) 에 보낸 것입니다.\n\n# 오케스트레이션 안내\n- 시작할 때 보고합니다.\n';

function answer(role: string, guide: { success: boolean; message?: string } = { success: true }) {
  request.mockImplementation((url: string) => {
    if (url === '/api/users/me') return Promise.resolve({ success: true, data: { role }, message: '' });
    if (url.startsWith('/api/orchestration/guide')) {
      return Promise.resolve(guide.success
        ? { success: true, data: { text: GUIDE, lead: null, updated: '2026-11-10' }, message: '' }
        : { success: false, data: null, message: guide.message || '' });
    }
    return Promise.reject(new Error(`unexpected ${url}`));
  });
}

function setup({ connected = true, member = false } = {}) {
  const paste = jest.fn().mockReturnValue(true);
  const handle = createRef<TerminalViewHandle>() as { current: TerminalViewHandle | null };
  handle.current = { paste } as unknown as TerminalViewHandle;
  render(
    <I18nProvider>
      <MemberContext.Provider value={member}>
        <AiGuideButton variant="head" sessionId="pty_1" handle={handle} connected={connected} />
      </MemberContext.Provider>
    </I18nProvider>,
  );
  return { paste };
}

const openMenu = async () => fireEvent.click(await screen.findByTestId('ai-guide-button'));

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  forgetGuides();
});

test('staff get the button and its menu, with what it is for', async () => {
  answer('admin');
  setup();
  await openMenu();
  const menu = screen.getByRole('menu', { name: 'AI 안내' });
  expect(menu).toBeInTheDocument();
  expect(screen.getByRole('menuitem', { name: '안내문 복사' })).toBeEnabled();
  expect(screen.getByRole('menuitem', { name: '이 세션에 붙여넣기' })).toBeEnabled();
  expect(screen.getByRole('menuitem', { name: '미리 보기' })).toBeEnabled();
  expect(menu).toHaveTextContent('협업 규칙');
  fireEvent.keyDown(document.activeElement || document.body, { key: 'Escape' });
  expect(screen.queryByRole('menu')).not.toBeInTheDocument();
});

test('a remote-only member sees no button', async () => {
  answer('remote');
  setup();
  await waitFor(() => expect(request).toHaveBeenCalledWith('/api/users/me'));
  await new Promise((r) => setTimeout(r, 0));
  expect(screen.queryByTestId('ai-guide-button')).not.toBeInTheDocument();
});

test('inside the member workspace it is not even asked', () => {
  answer('admin');
  setup({ member: true });
  expect(screen.queryByTestId('ai-guide-button')).not.toBeInTheDocument();
  expect(request).not.toHaveBeenCalled();
});

test('copy puts the fetched guide on the clipboard, fetched once a minute', async () => {
  answer('admin');
  const writeText = jest.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  Object.defineProperty(window, 'isSecureContext', { value: true, configurable: true });
  setup();
  await openMenu();
  fireEvent.click(screen.getByRole('menuitem', { name: '안내문 복사' }));
  await waitFor(() => expect(writeText).toHaveBeenCalledWith(GUIDE));
  expect(await screen.findByText('복사했어요')).toBeInTheDocument();
  expect(request).toHaveBeenCalledWith('/api/orchestration/guide?session=pty_1');
  fireEvent.click(screen.getByRole('menuitem', { name: '안내문 복사' }));
  await waitFor(() => expect(writeText).toHaveBeenCalledTimes(2));
  expect(request.mock.calls.filter(([u]) => String(u).startsWith('/api/orchestration/guide'))).toHaveLength(1);
});

test('paste hands the guide over as one bracketed paste, without Enter', async () => {
  answer('admin');
  const { paste } = setup();
  await openMenu();
  fireEvent.click(screen.getByRole('menuitem', { name: '이 세션에 붙여넣기' }));
  await waitFor(() => expect(paste).toHaveBeenCalledTimes(1));
  const sent: string = paste.mock.calls[0][0];
  expect(sent.startsWith('\x1b[200~')).toBe(true);
  expect(sent.endsWith('\x1b[201~')).toBe(true);
  expect(sent).toBe(`\x1b[200~${GUIDE.trimEnd()}\x1b[201~`);
  expect(sent).not.toMatch(/[\r\n]\x1b\[201~$/);
  expect(await screen.findByText(/Enter를 눌러 보내세요/)).toBeInTheDocument();
  expect(screen.queryByRole('menu')).not.toBeInTheDocument();
});

test('paste is offered only while the session is connected', async () => {
  answer('admin');
  const { paste } = setup({ connected: false });
  await openMenu();
  expect(screen.getByRole('menuitem', { name: '이 세션에 붙여넣기' })).toBeDisabled();
  expect(paste).not.toHaveBeenCalled();
});

test('a failed fetch shows an alert', async () => {
  answer('admin', { success: false, message: 'Session not found' });
  setup();
  await openMenu();
  fireEvent.click(screen.getByRole('menuitem', { name: '안내문 복사' }));
  const alert = await screen.findByRole('alert');
  expect(alert).toHaveTextContent('안내문을 불러오지 못했어요.');
  expect(alert).toHaveTextContent('Session not found');
});

test('preview shows the guide read-only, with its own copy button', async () => {
  answer('admin');
  setup();
  await openMenu();
  fireEvent.click(screen.getByRole('menuitem', { name: '미리 보기' }));
  const dialog = await screen.findByRole('dialog', { name: 'AI 안내문 미리 보기' });
  expect(await screen.findByTestId('ai-guide-text')).toHaveTextContent('오케스트레이션 안내');
  expect(dialog).toContainElement(screen.getByRole('button', { name: '안내문 복사' }));
});

test('the paste cannot be ended early by the text itself', () => {
  expect(bracketedPaste('a\r\nb\x1b[201~c\n\n')).toBe('\x1b[200~a\nb[201~c\x1b[201~');
});

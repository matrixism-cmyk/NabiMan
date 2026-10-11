import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from './i18n';
import App from './App';

jest.mock('./components/terminal/TerminalView', () => {
  const react = require('react');
  return { __esModule: true, default: react.forwardRef(function FakeTerminalView() {
    return react.createElement('div', { 'data-testid': 'fake-terminal' });
  }) };
});
const savedWindow = { id: 'tw_saved', title: 'local', subtitle: 'local', target: { kind: 'local' },
  x: 20, y: 20, w: 640, h: 400, z: 11, minimized: true, maximized: false };
let fetchMock: jest.SpyInstance;
beforeEach(() => {
  window.history.replaceState(null, '', '/');
  sessionStorage.clear(); localStorage.clear();
  localStorage.setItem('nabiman_term_windows', JSON.stringify([savedWindow]));
  fetchMock = jest.spyOn(global, 'fetch').mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    const data = url.endsWith('/api/users/me') ? {
      username: 'admin', role: 'admin', memberships: [], scope_restricted: false,
    } : url.endsWith('/provider') ? { enabled: false } : false;
    return { status: 200, ok: true, json: async () => ({ success: true, data }) } as Response;
  });
});
afterEach(() => { fetchMock.mockRestore(); });
const renderApp = () => render(<I18nProvider><App /></I18nProvider>);

test('logged-out page has signup and never restores saved terminals or requests terminal settings', async () => {
  renderApp();
  expect(screen.getByRole('button', { name: '회원가입 신청' })).toBeInTheDocument();
  expect(document.querySelector('.term-taskbar')).toBeNull();
  expect(screen.queryByTestId('fake-terminal')).not.toBeInTheDocument();
  await waitFor(() => expect(fetchMock).toHaveBeenCalled());
  expect(fetchMock.mock.calls.some(([url]) => String(url).includes('/api/terminal'))).toBe(false);
});
test('logout unmounts restored terminals while retaining that owner’s layout for the next login', async () => {
  sessionStorage.setItem('nabiman_token', 'test-session');
  renderApp();
  await screen.findByTestId('fake-terminal');
  expect(document.querySelector('.term-taskbar')).not.toBeNull();
  fireEvent.click(screen.getByRole('button', { name: /^(Logout|로그아웃)$/ }));
  await screen.findByRole('button', { name: '회원가입 신청' });
  expect(document.querySelector('.term-taskbar')).toBeNull();
  expect(screen.queryByTestId('fake-terminal')).not.toBeInTheDocument();
  expect(sessionStorage.getItem('nabiman_token')).toBeNull();
  expect(JSON.parse(localStorage.getItem('nabiman_term_windows:admin')!)).toHaveLength(1);
});

test('direct signup URL displays the application form without terminal controls', () => {
  window.history.replaceState(null, '', '/signup');
  renderApp();
  expect(screen.getByRole('heading', { name: '회원가입 신청' })).toBeInTheDocument();
  expect(screen.getByLabelText('가입 아이디')).toBeInTheDocument();
  expect(document.querySelector('.term-taskbar')).toBeNull();
});

import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import StartMenu from './StartMenu';

const mockRequest = jest.fn();
jest.mock('../../shared/api/useApi', () => ({ apiRequest: (path: string) => mockRequest(path) }));
jest.mock('./terminalStorage', () => ({ storedSessionId: (key: string) => (key === 'w1' ? 'pty_a' : '') }));

const server = { id: 'srv1', name: 'web-1', host: '10.0.0.1', port: 22, user: 'root', auth_method: 'key', tags: [], memo: '', created_at: '', last_checked: '', status: 'online' };
const sessions = [
  { id: 'pty_a', label: 'ssh:root@10.0.0.1:22', title: '배포 작업', alive: true, agents: ['claude'] },
  { id: 'pty_b', label: 'local', title: '', alive: true, busy: true },
  { id: 'pty_c', label: 'local', title: '끝난 것', alive: false },
];

beforeEach(() => {
  localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
  mockRequest.mockReset();
  mockRequest.mockImplementation((path: string) => Promise.resolve({ success: true, data: path === '/api/terminal/sessions' ? sessions : [server] }));
});

const show = (open = jest.fn()) => {
  render(<I18nProvider><StartMenu windows={[{ id: 'w1' }]} open={open} /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: /터미널 1/ }));
  return open;
};

test('lists the running sessions and the servers; ended sessions are left out', async () => {
  show();
  expect(await screen.findByText('배포 작업')).toBeInTheDocument();
  expect(screen.queryByText('끝난 것')).toBeNull();
  expect(screen.getByText('창 열림')).toBeInTheDocument();
  expect(screen.getByText('claude')).toBeInTheDocument();
  expect(screen.getAllByText('web-1').length).toBe(2); // under the session's name, and as a place to start a new one
});

test('a running session opens in a window joined to it, under its server name', async () => {
  const open = show();
  fireEvent.click(await screen.findByText('배포 작업'));
  expect(open).toHaveBeenCalledWith(expect.objectContaining({ kind: 'ssh', host: '10.0.0.1', serverId: 'srv1' }), { title: 'web-1', joinSessionId: 'pty_a' });
  expect(screen.queryByRole('menu')).toBeNull();
});

test('a server starts a new session; so does the local shell', async () => {
  const open = show();
  await screen.findByText('배포 작업');
  fireEvent.click(screen.getAllByText('web-1')[1]);
  expect(open).toHaveBeenLastCalledWith(expect.objectContaining({ kind: 'ssh', serverId: 'srv1' }), { title: 'web-1' });
  fireEvent.click(screen.getByRole('button', { name: /터미널 1/ }));
  const local = await waitFor(() => screen.getAllByText('로컬 셸'));
  fireEvent.click(local[local.length - 1]);
  expect(open).toHaveBeenLastCalledWith({ kind: 'local' }, { title: '로컬 셸' });
});

test('typing narrows both lists; Escape closes the menu', async () => {
  show();
  await screen.findByText('배포 작업');
  fireEvent.change(screen.getByLabelText('세션·서버 찾기'), { target: { value: '배포' } });
  expect(screen.getByText('배포 작업')).toBeInTheDocument();
  expect(screen.queryByText('로컬 셸')).toBeNull();
  fireEvent.keyDown(document, { key: 'Escape' });
  expect(screen.queryByRole('menu')).toBeNull();
});

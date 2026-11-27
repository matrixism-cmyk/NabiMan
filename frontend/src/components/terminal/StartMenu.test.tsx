import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import StartMenu from './StartMenu';
import { consoleKeeper } from '../remote/consoleKeeper';
import { consoleSnapshots } from '../remote/consoleSnapshots';

const mockRequest = jest.fn();
jest.mock('../../shared/api/useApi', () => ({ apiRequest: (path: string) => mockRequest(path) }));
jest.mock('./terminalStorage', () => ({ storedSessionId: (key: string) => (key === 'w1' ? 'pty_a' : '') }));

const server = { id: 'srv1', name: 'web-1', host: '10.0.0.1', port: 22, user: 'root', auth_method: 'key', tags: [], memo: '', created_at: '', last_checked: '', status: 'online' };
const store = { ...server, id: 'srv2', name: 'nas-files', host: '10.0.0.9', terminal: 'none', files: 'ftp', ftp_port: 21, status: 'offline' };
const consoles = [
  { name: 'win11', label: '윈도우 11', builtin: true, protocol: 'rdp' },
  { name: 'vm 2', label: 'ubuntu-desk', builtin: false, protocol: 'vnc', host: '10.0.5.2' },
];
let answers: Record<string, unknown> = {};
const sessions = [
  { id: 'pty_a', label: 'ssh:root@10.0.0.1:22', title: '배포 작업', alive: true, agents: ['claude'], preview: 'make deploy\nuploading 3/9\n' },
  { id: 'pty_b', label: 'local', title: '', alive: true, busy: true },
  { id: 'pty_c', label: 'local', title: '끝난 것', alive: false },
];

beforeEach(() => {
  localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
  mockRequest.mockReset();
  answers = { '/api/terminal/sessions': sessions, '/api/remote-servers': [server, store], '/api/remote/consoles': consoles };
  mockRequest.mockImplementation((path: string) => (answers[path]
    ? Promise.resolve({ success: true, data: answers[path] })
    : Promise.resolve({ success: false, data: null, message: 'forbidden' })));
  consoleKeeper.dropAll();
  consoleSnapshots.clear();
  window.history.replaceState(null, '', '/');
});
afterEach(() => { jest.restoreAllMocks(); });

const openConsole = jest.fn();
const show = (open = jest.fn(), windows: React.ComponentProps<typeof StartMenu>['windows'] = [{ id: 'w1' }]) => {
  openConsole.mockClear();
  render(<I18nProvider><StartMenu windows={windows} open={open} openConsole={openConsole} /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: new RegExp(`창 ${windows.length}`) }));
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
  fireEvent.click(screen.getByRole('button', { name: /창 1/ }));
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

test('a session card shows the last lines on its screen, or that there is none', async () => {
  show();
  const card = (await screen.findByText('배포 작업')).closest('button') as HTMLElement;
  expect(card.querySelector('pre')?.textContent).toBe('make deploy\nuploading 3/9');
  expect(screen.getAllByText('(화면 없음)').length).toBe(1);
  // What is on a screen finds its session too.
  fireEvent.change(screen.getByLabelText('세션·서버 찾기'), { target: { value: 'uploading' } });
  expect(screen.getByText('배포 작업')).toBeInTheDocument();
  expect(screen.queryByText('(화면 없음)')).toBeNull();
});

test('lists the VM consoles with how they are reached; the search narrows them too', async () => {
  show();
  const card = (await screen.findByText('윈도우 11')).closest('button') as HTMLElement;
  expect(card).toHaveTextContent('RDP');
  expect(card.querySelector('img')).toBeNull(); // never connected in this tab: no picture
  expect(screen.getByText('ubuntu-desk').closest('button')).toHaveTextContent('VNC');
  expect(screen.queryByText('연결 유지 중')).toBeNull();
  fireEvent.change(screen.getByLabelText('세션·서버 찾기'), { target: { value: 'ubuntu' } });
  expect(screen.queryByText('윈도우 11')).toBeNull();
  expect(screen.getByText('ubuntu-desk')).toBeInTheDocument();
});

test('a console card opens the console as a window on this page, as the rail\'s 분리 does', async () => {
  const opened = jest.spyOn(window, 'open');
  show();
  fireEvent.click(await screen.findByText('윈도우 11'));
  expect(openConsole).toHaveBeenLastCalledWith(expect.objectContaining({ name: 'win11', protocol: 'rdp' }));
  expect(screen.queryByRole('menu')).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: /창 1/ }));
  fireEvent.click(await screen.findByText('ubuntu-desk'));
  expect(openConsole).toHaveBeenLastCalledWith(expect.objectContaining({ name: 'vm 2' }));
  // No browser window: that is the card's other button.
  expect(opened).not.toHaveBeenCalled();
});

test('the card\'s small button — or Shift+click — is "새창으로 분리": this tab hangs up and a browser window opens', async () => {
  const close = jest.fn();
  consoleKeeper.adopt('win11', { close, alive: () => true, watch: () => () => undefined });
  const focus = jest.fn();
  const opened = jest.spyOn(window, 'open').mockReturnValue({ focus } as unknown as Window);
  show();
  await screen.findByText('윈도우 11');
  const button = screen.getByRole('menuitem', { name: '윈도우 11 새창으로 분리' });
  expect(button.title).toContain('다른 모니터');
  fireEvent.click(button);
  expect(close).toHaveBeenCalled();
  expect(opened).toHaveBeenLastCalledWith('/remote#rdp/win11', 'con_win11', expect.stringContaining('width=1300'));
  expect(focus).toHaveBeenCalled();
  expect(openConsole).not.toHaveBeenCalled();
  expect(screen.queryByRole('menu')).toBeNull();

  fireEvent.click(screen.getByRole('button', { name: /창 1/ }));
  fireEvent.click(await screen.findByText('ubuntu-desk'), { shiftKey: true });
  expect(opened).toHaveBeenLastCalledWith('/remote#console/vm%202', 'con_vm 2', expect.any(String));
  expect(openConsole).not.toHaveBeenCalled();
});

test('a console that has a window says so on its card, and the button counts consoles among the windows', async () => {
  show(jest.fn(), [{ id: 'w1' }, { id: 'c1', kind: 'console', console: { name: 'vm 2' } }]);
  const card = (await screen.findByText('ubuntu-desk')).closest('button') as HTMLElement;
  expect(card).toHaveTextContent('창 열림');
  expect((screen.getByText('윈도우 11').closest('button') as HTMLElement)).not.toHaveTextContent('창 열림');
});

test('no console section when the account may not list them', async () => {
  delete answers['/api/remote/consoles'];
  show();
  await screen.findByText('배포 작업');
  await waitFor(() => expect(mockRequest).toHaveBeenCalledWith('/api/remote/consoles'));
  expect(screen.queryByText(/VM 콘솔/)).toBeNull();
  mockRequest.mockImplementation((path: string) => (path === '/api/remote/consoles' ? Promise.reject(new Error('offline')) : Promise.resolve({ success: true, data: answers[path] })));
  fireEvent.click(screen.getByRole('button', { name: /창 1/ }));
  fireEvent.click(screen.getByRole('button', { name: /창 1/ }));
  await screen.findByText('배포 작업');
  expect(screen.queryByText(/VM 콘솔/)).toBeNull();
});

test('a console connected in this tab shows its screen and that it is kept', async () => {
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({ drawImage: jest.fn() } as unknown as CanvasRenderingContext2D);
  jest.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockReturnValue('data:image/jpeg;base64,AAAA');
  const screenBox = document.createElement('div');
  screenBox.appendChild(Object.assign(document.createElement('canvas'), { width: 1280, height: 800 }));
  consoleKeeper.adopt('vm 2', { close: jest.fn(), alive: () => true, watch: () => () => undefined, status: 'connected', screen: screenBox } as never);
  show();
  const card = (await screen.findByText('ubuntu-desk')).closest('button') as HTMLElement;
  await waitFor(() => expect(card.querySelector('img')?.getAttribute('src')).toBe('data:image/jpeg;base64,AAAA'));
  expect(card).toHaveTextContent('연결 유지 중');
  expect(card).not.toHaveTextContent('마지막으로 본 화면');
  expect(JSON.stringify({ ...localStorage }) + JSON.stringify({ ...sessionStorage })).not.toContain('AAAA');
});

test('a files-only server is listed under 파일 and opens in the remote workspace', async () => {
  const asked = jest.fn();
  window.addEventListener('nabiman_remote_select', asked);
  show();
  fireEvent.click(await screen.findByText('nas-files'));
  window.removeEventListener('nabiman_remote_select', asked);
  expect(asked.mock.calls[0][0].detail).toEqual({ kind: 'server', id: 'srv2' });
  expect(JSON.parse(localStorage.getItem('nabiman_remote_selection') || '{}')).toEqual({ kind: 'server', id: 'srv2' });
  expect(window.location.hash).toBe('#remote/remote');
  expect(screen.queryByRole('menu')).toBeNull();
});

test('arrow keys move between entries; a letter goes back to the search box', async () => {
  show();
  await screen.findByText('배포 작업');
  const search = screen.getByLabelText('세션·서버 찾기');
  const items = screen.getAllByRole('menuitem');
  search.focus();
  fireEvent.keyDown(search, { key: 'ArrowDown' });
  expect(document.activeElement).toBe(items[0]);
  fireEvent.keyDown(items[0], { key: 'ArrowRight' });
  expect(document.activeElement).toBe(items[1]);
  fireEvent.keyDown(items[1], { key: 'ArrowDown' });
  expect(document.activeElement).toBe(items[2]);
  fireEvent.keyDown(items[2], { key: 'End' });
  expect(document.activeElement).toBe(items[items.length - 1]);
  fireEvent.keyDown(items[items.length - 1], { key: 'Home' });
  fireEvent.keyDown(items[0], { key: 'ArrowUp' });
  expect(document.activeElement).toBe(search);
  items[1].focus();
  fireEvent.keyDown(items[1], { key: 'w' });
  expect(document.activeElement).toBe(search);
});

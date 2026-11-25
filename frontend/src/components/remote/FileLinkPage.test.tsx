import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import FileLinkPage from './FileLinkPage';
import TerminalWithFiles from './TerminalWithFiles';
import { TerminalFilesContext } from './terminalFiles';
import * as api from './sftp/sftpApi';

jest.mock('./sftp/sftpApi', () => ({
  ...jest.requireActual('./sftp/sftpApi'), listDir: jest.fn(), downloadFile: jest.fn(),
}));
jest.mock('../../shared/api/useApi', () => ({
  ...jest.requireActual('../../shared/api/useApi'),
  useApi: jest.fn(),
}));
const { useApi } = jest.requireMock('../../shared/api/useApi') as { useApi: jest.Mock };
const SERVERS = { data: [{ id: 'srv_1', name: 'web-01' }], loading: false, error: '', refetch: () => undefined };
const mocked = api as jest.Mocked<typeof api>;

const file = (name: string) => ({ name, kind: 'file', to_dir: false, size: 12, mtime: 1_700_000_000, mode: 0o644 });
const DECK = { path: '/work/deck', parent: '/work', truncated: false, entries: [file('a.txt'), file('.env'), file('z.md')] } as api.SftpListing;
const HOME = { path: '/home/me', parent: '/home', truncated: false, entries: [file('home.txt')] } as api.SftpListing;

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem('nabiman_lang', 'ko');
  localStorage.setItem('nabiman_sftp_path_srv_1', '/home/me');
  useApi.mockReturnValue(SERVERS);
  mocked.listDir.mockImplementation(async (_src, path) => (path === '/work/deck' ? { success: true, data: DECK, message: '' }
    : path === '/work/gone' ? { success: false, data: null, message: 'No such file' }
      : { success: true, data: HOME, message: '' }));
  mocked.downloadFile.mockImplementation(() => ({ promise: new Promise(() => undefined), abort: jest.fn() }));
});

const page = (path: string) => render(<I18nProvider><FileLinkPage link={{ scope: 'srv_1', path }} /></I18nProvider>);

test('a link opens the server\'s file panel at the folder, with the file shown over it', async () => {
  page('/work/deck/z.md');
  const dialog = await screen.findByRole('dialog');
  expect(dialog).toHaveTextContent('z.md');
  expect(mocked.listDir).toHaveBeenCalledWith(expect.objectContaining({ base: '/api/terminal/sftp/srv_1' }), '/work/deck');
  expect(mocked.listDir).not.toHaveBeenCalledWith(expect.anything(), '/home/me');
  expect(screen.getByLabelText('web-01 파일')).toBeInTheDocument();
  expect(screen.getByText('a.txt')).toBeInTheDocument();
  // The other files of the folder can be stepped to, as from the panel.
  expect(dialog).toHaveTextContent('2 / 2');
  expect(document.title).toBe('z.md · NabiMan');
});

test('a file the list hides is still shown', async () => {
  page('/work/deck/.env');
  expect(await screen.findByRole('dialog')).toHaveTextContent('.env');
});

test('a file that is gone says so, and the server\'s refusal is what an account without access sees', async () => {
  const { unmount } = page('/work/deck/nope.pdf');
  expect(await screen.findByRole('alert')).toHaveTextContent('nope.pdf 파일이 이 폴더에 없습니다');
  expect(screen.queryByRole('dialog')).toBeNull();
  unmount();
  mocked.listDir.mockResolvedValue({ success: false, data: null, message: 'Saved server not found' });
  page('/work/deck/z.md');
  await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument());
  expect(screen.queryByRole('dialog')).toBeNull();
});

test('a terminal with a file panel offers its files to the terminal inside; one without does not', async () => {
  const seen: Array<string | null> = [];
  const Probe = () => (
    <TerminalFilesContext.Consumer>{(files) => { seen.push(files ? files.source.base : null); return null; }}</TerminalFilesContext.Consumer>
  );
  const ui = (target: Parameters<typeof TerminalWithFiles>[0]['target']) => render(
    <I18nProvider><TerminalWithFiles target={target} name="x" place="window" onInsert={jest.fn()}><Probe /></TerminalWithFiles></I18nProvider>);
  ui({ kind: 'local' }).unmount();
  ui({ kind: 'ssh', serverId: 'srv_1' }).unmount();
  ui({ kind: 'ssh', serverId: 'srv_2', noFiles: true }).unmount();
  ui({ kind: 'ssh', host: 'adhoc.example' }).unmount();
  expect(Array.from(new Set(seen))).toEqual(['/api/terminal/sftp/local', '/api/terminal/sftp/srv_1', null]);
});

import React from 'react';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { I18nProvider } from '../../../i18n';
import SftpPanel from './SftpPanel';
import TerminalWithFiles from '../TerminalWithFiles';
import { SFTP_MIME } from './sftpDrop';
import * as api from './sftpApi';

jest.mock('./sftpApi', () => {
  const actual = jest.requireActual('./sftpApi');
  return {
    ...actual,
    listDir: jest.fn(), makeDir: jest.fn(), movePath: jest.fn(), removePath: jest.fn(),
    uploadFile: jest.fn(), downloadFile: jest.fn(), downloadLink: jest.fn(),
  };
});

const mocked = api as jest.Mocked<typeof api>;
const ok = <T,>(data: T) => Promise.resolve({ success: true, data, message: '' });

const HOME = {
  path: '/home/me', parent: '/home', truncated: false,
  entries: [
    { name: 'logs', kind: 'dir', to_dir: false, size: 4096, mtime: 1_700_000_000, mode: 0o755 },
    { name: 'app.conf', kind: 'file', to_dir: false, size: 2048, mtime: 1_700_000_000, mode: 0o644 },
    { name: '.bashrc', kind: 'file', to_dir: false, size: 10, mtime: 1_700_000_000, mode: 0o644 },
  ],
} as api.SftpListing;

function renderPanel(onInsert = jest.fn()) {
  render(
    <I18nProvider>
      <SftpPanel serverId="srv_1" serverName="web-01" onInsert={onInsert} onClose={() => {}} />
    </I18nProvider>,
  );
  return onInsert;
}

/** jsdom has no DataTransfer; this carries just what the panel reads. */
function transfer(data: Record<string, string>, files: File[] = []) {
  const types = [...Object.keys(data), ...(files.length ? ['Files'] : [])];
  return {
    types, files, items: files.map((f) => ({ kind: 'file', getAsFile: () => f })),
    getData: (type: string) => data[type] || '', setData: jest.fn(), dropEffect: '', effectAllowed: '',
  };
}

const row = (name: string) => screen.getByText(name).closest('.sftp-row') as HTMLElement;

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem('nabiman_lang', 'ko');
  window.confirm = jest.fn(() => true);
  mocked.listDir.mockImplementation((_id, path) => ok(path === '/home/me/logs'
    ? { path: '/home/me/logs', parent: '/home/me', truncated: false, entries: [] }
    : HOME));
  mocked.movePath.mockImplementation((_id, _from, to) => ok(to));
  mocked.removePath.mockImplementation(() => ok(1));
  mocked.makeDir.mockImplementation((_id, path) => ok(path));
  mocked.uploadFile.mockImplementation((_id, _dir, _name, file) => ({
    promise: Promise.resolve({ ok: true }), abort: jest.fn(), size: (file as Blob).size,
  }) as unknown as api.Transfer);
});

describe('SFTP panel', () => {
  it('starts at home, folders first, hidden files hidden', async () => {
    renderPanel();
    await screen.findByText('logs');
    expect(mocked.listDir).toHaveBeenCalledWith('srv_1', '');
    const names = Array.from(document.querySelectorAll('.sftp-row .sftp-name')).map((n) => n.textContent);
    expect(names).toEqual(['..', 'logs', 'app.conf']);
    expect(localStorage.getItem('nabiman_sftp_path_srv_1')).toBe('/home/me');
  });

  it('opens a folder on double click', async () => {
    renderPanel();
    fireEvent.doubleClick(row(await screen.findByText('logs').then(() => 'logs')));
    await waitFor(() => expect(mocked.listDir).toHaveBeenLastCalledWith('srv_1', '/home/me/logs'));
    expect(await screen.findByText(/빈 폴더입니다/)).toBeInTheDocument();
  });

  it('moves a row dropped onto a folder', async () => {
    renderPanel();
    await screen.findByText('logs');
    const dt = transfer({ [SFTP_MIME]: JSON.stringify(['/home/me/app.conf']), 'text/plain': '/home/me/app.conf' });
    fireEvent.dragOver(row('logs'), { dataTransfer: dt });
    expect(row('logs').className).toContain('is-drop');
    fireEvent.drop(row('logs'), { dataTransfer: dt });
    await waitFor(() => expect(mocked.movePath).toHaveBeenCalledWith('srv_1', '/home/me/app.conf', '/home/me/logs/app.conf'));
  });

  it('uploads desktop files into the folder they are dropped on', async () => {
    renderPanel();
    await screen.findByText('logs');
    const file = new File(['hello'], 'notes.txt', { type: 'text/plain' });
    await act(async () => {
      fireEvent.drop(row('logs'), { dataTransfer: transfer({}, [file]) });
    });
    await waitFor(() => expect(mocked.uploadFile).toHaveBeenCalled());
    const [id, dir, name, body, overwrite] = mocked.uploadFile.mock.calls[0];
    expect([id, dir, name, body, overwrite]).toEqual(['srv_1', '/home/me/logs', 'notes.txt', file, false]);
  });

  it('asks before deleting, and deletes nothing when told no', async () => {
    renderPanel();
    await screen.findByText('app.conf');
    fireEvent.click(row('app.conf'));
    (window.confirm as jest.Mock).mockReturnValueOnce(false);
    fireEvent.keyDown(document.querySelector('.sftp-body') as HTMLElement, { key: 'Delete' });
    expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining('app.conf'));
    expect(mocked.removePath).not.toHaveBeenCalled();

    fireEvent.keyDown(document.querySelector('.sftp-body') as HTMLElement, { key: 'Delete' });
    await waitFor(() => expect(mocked.removePath).toHaveBeenCalledWith('srv_1', '/home/me/app.conf'));
  });

  it('renames with F2', async () => {
    renderPanel();
    await screen.findByText('app.conf');
    fireEvent.click(row('app.conf'));
    fireEvent.keyDown(document.querySelector('.sftp-body') as HTMLElement, { key: 'F2' });
    const input = document.querySelector('.sftp-edit') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'app.conf.bak' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(mocked.movePath).toHaveBeenCalledWith('srv_1', '/home/me/app.conf', '/home/me/app.conf.bak'));
  });

  it('offers a file to the desktop once its link is ready', async () => {
    mocked.downloadLink.mockImplementation(() => Promise.resolve('https://man.example/api/terminal/sftp/srv_1/download?path=x&ticket=t'));
    renderPanel();
    await screen.findByText('app.conf');
    await act(async () => { fireEvent.pointerDown(row('app.conf')); });
    expect(mocked.downloadLink).toHaveBeenCalledWith('srv_1', '/home/me/app.conf');
    const dt = transfer({});
    fireEvent.dragStart(row('app.conf'), { dataTransfer: dt });
    expect(dt.setData).toHaveBeenCalledWith('DownloadURL',
      'application/octet-stream:app.conf:https://man.example/api/terminal/sftp/srv_1/download?path=x&ticket=t');

    // Folders cannot be saved that way, and need no link.
    fireEvent.pointerDown(row('logs'));
    expect(mocked.downloadLink).toHaveBeenCalledTimes(1);
  });

  it('types quoted paths into the terminal from the menu', async () => {
    const onInsert = renderPanel();
    mocked.listDir.mockImplementation(() => ok({ ...HOME, entries: [
      { name: 'my file.txt', kind: 'file', to_dir: false, size: 1, mtime: null, mode: null }] } as api.SftpListing));
    fireEvent.click(screen.getByRole('button', { name: '새로고침' }));
    fireEvent.contextMenu(row(await screen.findByText('my file.txt').then(() => 'my file.txt')));
    fireEvent.click(screen.getByRole('menuitem', { name: '터미널에 경로 입력' }));
    expect(onInsert).toHaveBeenCalledWith("'/home/me/my file.txt' ");
  });
});

describe('terminal with files', () => {
  it('types the paths of rows dropped onto the terminal', async () => {
    const onInsert = jest.fn();
    render(
      <I18nProvider>
        <TerminalWithFiles target={{ kind: 'ssh', serverId: 'srv_1' }} name="web-01" onInsert={onInsert}>
          <div data-testid="terminal" />
        </TerminalWithFiles>
      </I18nProvider>,
    );
    await screen.findByText('logs');
    const term = screen.getByTestId('terminal').parentElement as HTMLElement;
    const dt = transfer({ [SFTP_MIME]: '["/home/me/app.conf"]', 'text/plain': '/home/me/app.conf' });
    fireEvent.dragOver(term, { dataTransfer: dt });
    fireEvent.drop(term, { dataTransfer: dt });
    expect(onInsert).toHaveBeenCalledWith('/home/me/app.conf ');
  });

  it('folds away to a tab and remembers it', async () => {
    render(
      <I18nProvider>
        <TerminalWithFiles target={{ kind: 'ssh', serverId: 'srv_1' }} name="web-01" onInsert={jest.fn()}>
          <div data-testid="terminal" />
        </TerminalWithFiles>
      </I18nProvider>,
    );
    await screen.findByText('logs');
    fireEvent.click(screen.getByRole('button', { name: '파일 패널 닫기' }));
    expect(document.querySelector('.sftp')).toBeNull();
    expect(localStorage.getItem('nabiman_sftp_open')).toBe('0');
    fireEvent.click(screen.getByRole('button', { name: /파일/ }));
    expect(await screen.findByText('logs')).toBeInTheDocument();
  });

  it('shows no panel for an SSH target that is not saved', () => {
    render(
      <I18nProvider>
        <TerminalWithFiles target={{ kind: 'ssh', host: 'h', user: 'u' }} name="h" onInsert={jest.fn()}>
          <div data-testid="terminal" />
        </TerminalWithFiles>
      </I18nProvider>,
    );
    expect(screen.getByTestId('terminal')).toBeInTheDocument();
    expect(document.querySelector('.rw-split')).toBeNull();
  });

  it('shows this machine\'s files beside the local shell', async () => {
    render(
      <I18nProvider>
        <TerminalWithFiles target={{ kind: 'local' }} name="로컬 셸" onInsert={jest.fn()}>
          <div data-testid="terminal" />
        </TerminalWithFiles>
      </I18nProvider>,
    );
    await screen.findByText('logs');
    expect(mocked.listDir).toHaveBeenCalledWith('local', '');
  });

  it('starts folded in a floating window', () => {
    render(
      <I18nProvider>
        <TerminalWithFiles target={{ kind: 'local' }} name="로컬 셸" place="window" onInsert={jest.fn()}>
          <div data-testid="terminal" />
        </TerminalWithFiles>
      </I18nProvider>,
    );
    expect(document.querySelector('.sftp')).toBeNull();
    expect(document.querySelector('.rw-split-tab')).toBeTruthy();
  });
});

import React, { useRef } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { Terminal } from 'xterm';
import { I18nProvider } from '../../../i18n';
import { TerminalFilesContext } from '../../remote/terminalFiles';
import { accountSource, downloadFile, listDir, saveFile, SftpSource, shareSource } from '../../remote/sftp/sftpApi';
import { copyText } from '../ClaudeAddress';
import { HOLD_MS } from './filePress';
import { forgetFiles } from './fileResolve';
import { useTerminalFileLinks } from './TerminalFileLinks';

jest.mock('../../remote/sftp/sftpApi', () => ({
  ...jest.requireActual('../../remote/sftp/sftpApi'), listDir: jest.fn(), downloadFile: jest.fn(), saveFile: jest.fn(),
}));
jest.mock('../ClaudeAddress', () => ({ copyText: jest.fn() }));
const list = listDir as jest.Mock;
const copy = copyText as jest.Mock;

const PATH = '/work/deck/강의자료.pdf';
const entries = [
  { name: '강의자료.pdf', kind: 'file', to_dir: false, size: 694682, mtime: 1, mode: 0o644 },
  { name: 'notes.txt', kind: 'file', to_dir: false, size: 5, mtime: 1, mode: 0o644 },
  { name: 'sub', kind: 'dir', to_dir: false, size: 0, mtime: 1, mode: 0o755 },
];

/** A terminal 40 × 10 cells of 10 × 20 px, with `text` on it. */
async function terminal(text: string): Promise<{ term: Terminal; el: HTMLDivElement }> {
  const term = new Terminal({ cols: 40, rows: 10, allowProposedApi: true });
  await new Promise<void>((done) => term.write(text, done));
  const el = document.createElement('div');
  const element = document.createElement('div');
  const grid = document.createElement('div');
  grid.className = 'xterm-screen';
  grid.getBoundingClientRect = () => ({ left: 0, top: 0, width: 400, height: 200, right: 400, bottom: 200, x: 0, y: 0, toJSON: () => ({}) });
  element.appendChild(grid);
  el.appendChild(element);
  document.body.appendChild(el);
  Object.defineProperty(term, 'element', { value: element });
  return { term, el };
}

function Harness({ term, el }: { term: Terminal; el: HTMLDivElement }) {
  const termRef = useRef<Terminal | null>(term);
  const containerRef = useRef<HTMLDivElement | null>(el);
  return <>{useTerminalFileLinks({ termRef, containerRef, storageKey: 'pane', sessionId: () => '' })}</>;
}

async function mount(text: string, source: SftpSource | null = accountSource('local'), showFolder = jest.fn()) {
  const { term, el } = await terminal(text);
  jest.useFakeTimers(); // after the write: xterm takes its input on a timer
  const grid = el.querySelector('.xterm-screen') as HTMLElement;
  const seen: string[] = [];
  grid.addEventListener('mousedown', () => seen.push('mousedown'));
  render(
    <I18nProvider>
      <TerminalFilesContext.Provider value={source ? { source, showFolder } : null}>
        <Harness term={term} el={el} />
      </TerminalFilesContext.Provider>
    </I18nProvider>,
  );
  /** Presses on column `col` (from 0) of the first row and holds. */
  const hold = async (col: number) => {
    fireEvent.mouseDown(grid, { clientX: col * 10 + 5, clientY: 10, button: 0, detail: 1 });
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    await act(async () => { jest.advanceTimersByTime(HOLD_MS); });
  };
  return { term, el, grid, seen, hold, showFolder };
}

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  forgetFiles();
  copy.mockResolvedValue(true);
  list.mockImplementation(async (_src: SftpSource, path: string) => (path === '/work/deck'
    ? { success: true, data: { path, parent: '/work', entries, truncated: false }, message: '' }
    : { success: false, data: null, message: 'No such file' }));
  (downloadFile as jest.Mock).mockImplementation(() => ({ promise: new Promise(() => undefined), abort: jest.fn() }));
});
afterEach(() => { jest.useRealTimers(); document.body.innerHTML = ''; });

describe('a path held down in a terminal', () => {
  it('opens a menu named after the file, whose "열기" shows it in the preview', async () => {
    const { hold, seen } = await mount(`at ${PATH}`);
    await hold(10);
    expect(screen.getByRole('menu', { name: '강의자료.pdf' })).toBeInTheDocument();
    expect(screen.getAllByRole('menuitem').map((b) => b.textContent)).toEqual(['👁열기', '⧉경로 복사', '🔗링크 복사']);
    expect(list).toHaveBeenCalledTimes(1);
    expect(list).toHaveBeenCalledWith(expect.objectContaining({ base: '/api/terminal/sftp/local' }), '/work/deck');
    expect(seen).toEqual([]); // the terminal was not given the press

    fireEvent.click(screen.getByRole('menuitem', { name: /열기/ }));
    expect(screen.queryByRole('menu')).toBeNull();
    const dialog = screen.getByRole('dialog');
    expect(dialog).toHaveTextContent('강의자료.pdf');
    expect(dialog).toHaveTextContent('678 KB');
    expect(downloadFile).toHaveBeenCalledWith(expect.objectContaining({ key: 'local' }), PATH, expect.any(Function));
    // One file: nothing to step to; the download button saves this file.
    expect(screen.queryByLabelText('다음')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: /내려받기/ }));
    expect(saveFile).toHaveBeenCalledWith(expect.objectContaining({ key: 'local' }), PATH, '강의자료.pdf');
  });

  it('copies the absolute path, says so, and closes', async () => {
    const { hold } = await mount(`at ${PATH}`);
    await hold(10);
    await act(async () => { fireEvent.click(screen.getByRole('menuitem', { name: /경로 복사/ })); });
    expect(copy).toHaveBeenCalledWith(PATH);
    expect(screen.getByRole('status')).toHaveTextContent('복사했습니다');
    await act(async () => { jest.advanceTimersByTime(1000); });
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('copies a link that names the server and the file and carries no key', async () => {
    const { hold } = await mount(`at ${PATH}`, accountSource('srv-7'));
    await hold(10);
    await act(async () => { fireEvent.click(screen.getByRole('menuitem', { name: /링크 복사/ })); });
    expect(copy).toHaveBeenCalledWith(`http://localhost/remote#file/srv-7/work/deck/${encodeURIComponent('강의자료.pdf')}`);
  });

  it('offers a folder its path and the file panel, not a preview or a link', async () => {
    const { hold, showFolder } = await mount('ls /work/deck/sub');
    await hold(6);
    expect(screen.getAllByRole('menuitem').map((b) => b.textContent)).toEqual(['📂파일 패널에서 열기', '⧉경로 복사']);
    fireEvent.click(screen.getByRole('menuitem', { name: /파일 패널/ }));
    expect(showFolder).toHaveBeenCalledWith('/work/deck/sub');
  });

  it('does nothing for a path that is not there: the press goes to the terminal', async () => {
    const { hold, seen } = await mount('at /work/deck/gone.pdf');
    await hold(10);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(seen).toEqual(['mousedown']);
  });

  it('finds a bare name in the folder the prompt above it shows, not the one a later prompt shows', async () => {
    const { grid } = await mount('nabi@box:/work/deck$ ls\r\nnotes.txt\r\nnabi@box:/work/deck$ cd /\r\nnabi@box:/$ echo notes.txt\r\nnotes.txt');
    // Row 4, printed under `/`: asked about there (and at home), and not found.
    fireEvent.mouseDown(grid, { clientX: 15, clientY: 90, button: 0, detail: 1 });
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    await act(async () => { jest.advanceTimersByTime(HOLD_MS); });
    expect(screen.queryByRole('menu')).toBeNull();
    expect(list.mock.calls.map(([, path]) => path)).toEqual(['', '/']);
    // Row 1, printed under `/work/deck`.
    fireEvent.mouseUp(grid);
    list.mockClear();
    await act(async () => { fireEvent.mouseDown(grid, { clientX: 15, clientY: 30, button: 0, detail: 1 }); });
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    await act(async () => { jest.advanceTimersByTime(HOLD_MS); });
    expect(screen.getByRole('menu', { name: 'notes.txt' })).toBeInTheDocument();
    expect(list.mock.calls.map(([, path]) => path)).toEqual(['', '/work/deck']);
  });

  it('does nothing over plain text, and asks the server nothing', async () => {
    const { hold, seen } = await mount(`at ${PATH}`);
    await hold(1);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(list).not.toHaveBeenCalled();
    expect(seen).toEqual(['mousedown']);
  });

  it('is not there at all without file access', async () => {
    const { hold, seen, term } = await mount(`at ${PATH}`, null);
    const provider = jest.spyOn(term, 'registerLinkProvider');
    await hold(10);
    expect(screen.queryByRole('menu')).toBeNull();
    expect(list).not.toHaveBeenCalled();
    expect(provider).not.toHaveBeenCalled();
    expect(seen).toEqual(['mousedown']);
  });

  it('gives a share link\'s visitor no link to copy', async () => {
    const { hold } = await mount(`at ${PATH}`, shareSource('tok', 'key', false));
    await hold(10);
    expect(screen.getAllByRole('menuitem').map((b) => b.textContent)).toEqual(['👁열기', '⧉경로 복사']);
  });

  it('closes the menu when the terminal is resized or the wheel turns', async () => {
    const { hold, term, grid } = await mount(`at ${PATH}`);
    await hold(10);
    await act(async () => { term.resize(50, 10); });
    expect(screen.queryByRole('menu')).toBeNull();
    await hold(10);
    expect(screen.getByRole('menu')).toBeInTheDocument();
    await act(async () => { fireEvent.wheel(grid); });
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('underlines paths for the pointer without asking the server', async () => {
    const { term } = await mount(`at ${PATH}`);
    const provider = (term as unknown as { _core: { linkifier2: { _linkProviders: Array<{ provideLinks: (y: number, cb: (links?: unknown[]) => void) => void }> } } })
      ._core.linkifier2._linkProviders.slice(-1)[0];
    const got = jest.fn();
    provider.provideLinks(1, got);
    expect(got).toHaveBeenCalledWith([expect.objectContaining({ text: PATH, decorations: { underline: true, pointerCursor: false } })]);
    provider.provideLinks(2, got);
    expect(got).toHaveBeenLastCalledWith(undefined);
    expect(list).not.toHaveBeenCalled();
  });
});

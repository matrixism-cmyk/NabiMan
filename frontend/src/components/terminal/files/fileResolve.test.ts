import { forgetFiles, normalize, resolveFile } from './fileResolve';
import type { PathMatch } from './filePaths';
import { accountSource, shareSource } from '../../remote/sftp/sftpApi';

jest.mock('../../../shared/api/useApi', () => ({
  ...jest.requireActual('../../../shared/api/useApi'),
  apiRequest: jest.fn(),
}));
const { apiRequest } = jest.requireMock('../../../shared/api/useApi') as { apiRequest: jest.Mock };

const file = (name: string, kind = 'file') => ({ name, kind, to_dir: false, size: 10, mtime: 1, mode: 0o644 });
const tree: Record<string, ReturnType<typeof file>[]> = {
  '/home/nabi': [file('.bashrc'), file('notes.md')],
  '/work/proj': [file('report.pdf'), file('src', 'dir')],
  '/work/proj/src': [file('app.ts')],
  '/panel': [file('panel.txt')],
  '/srv/shell': [file('shell.txt'), file('both.txt')],
  '/srv/prompt': [file('prompt.txt'), file('both.txt')],
  '/home/nabi/sub': [file('insub.txt')],
};
const match = (path: string, kind: PathMatch['kind']): PathMatch => ({ start: 0, end: path.length, path, kind });

describe('resolving a printed path', () => {
  let asked: string[];
  let cwd: string | null;
  beforeEach(() => {
    cwd = null;
    forgetFiles();
    localStorage.clear();
    asked = [];
    apiRequest.mockImplementation(async (url: string) => {
      asked.push(url);
      if (url.endsWith('/claude')) return { success: true, data: { claude: url.includes('plain') ? [] : [{ cwd: '/work/proj' }] } };
      if (url.endsWith('/cwd')) return { success: true, data: { cwd } };
      const dir = decodeURIComponent(url.split('path=')[1]) || '/home/nabi';
      return tree[dir] ? { success: true, data: { path: dir, parent: null, entries: tree[dir], truncated: false } }
        : { success: false, data: null, message: 'No such file' };
    });
  });
  const lists = () => asked.filter((u) => u.includes('/list?'));
  const local = accountSource('local');

  it('asks for the folder of an absolute path once, and only this server', async () => {
    const found = await resolveFile([match('/work/proj/report.pdf', 'absolute')], { source: local });
    expect(found).toMatchObject({ path: '/work/proj/report.pdf', dir: '/work/proj', folder: false });
    expect(asked).toEqual(['/api/terminal/sftp/local/list?path=%2Fwork%2Fproj']);
    // Again within a few seconds: the answer is kept.
    await resolveFile([match('/work/proj/report.pdf', 'absolute')], { source: local });
    expect(asked).toHaveLength(1);
  });

  it('tells a folder from a file, and something missing from both', async () => {
    expect(await resolveFile([match('/work/proj/src', 'absolute')], { source: local })).toMatchObject({ folder: true });
    expect(await resolveFile([match('/work/proj/gone.txt', 'absolute')], { source: local })).toBeNull();
    expect(await resolveFile([match('/nowhere/x.txt', 'absolute')], { source: local })).toBeNull();
  });

  it('reads ~ as the account\'s home there', async () => {
    expect(await resolveFile([match('~/notes.md', 'home')], { source: local })).toMatchObject({ path: '/home/nabi/notes.md' });
  });

  it('looks for a bare name where Claude works, then where the file panel is, then at home', async () => {
    localStorage.setItem('nabiman_sftp_path_local', '/panel');
    const look = { source: local, sessionId: 'sess-1' };
    expect(await resolveFile([match('report.pdf', 'name')], look)).toMatchObject({ path: '/work/proj/report.pdf' });
    expect(asked[0]).toBe('/api/terminal/sessions/sess-1/claude');
    expect(await resolveFile([match('src/app.ts', 'relative')], look)).toMatchObject({ path: '/work/proj/src/app.ts' });
    expect(await resolveFile([match('b/src/app.ts', 'relative')], look)).toMatchObject({ path: '/work/proj/src/app.ts' });
    expect(await resolveFile([match('panel.txt', 'name')], look)).toMatchObject({ path: '/panel/panel.txt' });
    expect(await resolveFile([match('notes.md', 'name')], look)).toMatchObject({ path: '/home/nabi/notes.md' });
    expect(await resolveFile([match('nope.md', 'name')], look)).toBeNull();
    // Each folder is asked about once: home, the three real ones, and three that are not there.
    expect(lists()).toHaveLength(7);
  });

  it('asks the server where a plain shell is, afresh for a later press', async () => {
    const look = { source: local, sessionId: 'plain-1' };
    cwd = '/srv/shell';
    expect(await resolveFile([match('shell.txt', 'name')], look)).toMatchObject({ path: '/srv/shell/shell.txt' });
    expect(asked.slice(0, 2)).toEqual(['/api/terminal/sessions/plain-1/claude', '/api/terminal/sessions/plain-1/cwd']);
    // Home is asked for as before (the last place to look); the shell's folder is the first listed.
    expect(lists()).toEqual(['/api/terminal/sftp/local/list?path=', '/api/terminal/sftp/local/list?path=%2Fsrv%2Fshell']);
    // `cd /` since: the kept answer is not used once the press is over.
    cwd = '/';
    jest.spyOn(Date, 'now').mockReturnValue(Date.now() + 1500);
    expect(await resolveFile([match('shell.txt', 'name')], look)).toBeNull();
    expect(asked.filter((u) => u.endsWith('/cwd'))).toHaveLength(2);
    (Date.now as jest.Mock).mockRestore();
  });

  it('looks under the prompt first, then Claude, then the shell, and reads ~ as home', async () => {
    cwd = '/srv/shell';
    const look = { source: local, sessionId: 'sess-1', promptDir: '/srv/prompt' };
    expect(await resolveFile([match('both.txt', 'name')], look)).toMatchObject({ path: '/srv/prompt/both.txt' });
    expect(await resolveFile([match('report.pdf', 'name')], look)).toMatchObject({ path: '/work/proj/report.pdf' });
    expect(await resolveFile([match('shell.txt', 'name')], look)).toMatchObject({ path: '/srv/shell/shell.txt' });
    expect(await resolveFile([match('insub.txt', 'name')], { ...look, promptDir: '~/sub' })).toMatchObject({ path: '/home/nabi/sub/insub.txt' });
    expect(await resolveFile([match('notes.md', 'name')], { source: local, promptDir: '~' })).toMatchObject({ path: '/home/nabi/notes.md' });
    // What a screen shows is only ever a folder to list: nothing but a path gets through.
    forgetFiles();
    asked = [];
    expect(await resolveFile([match('x.txt', 'name')], { source: local, promptDir: '/srv/prompt/../../etc' })).toBeNull();
    expect(await resolveFile([match('x.txt', 'name')], { source: local, promptDir: 'relative' })).toBeNull();
    expect(lists()).toEqual(['/api/terminal/sftp/local/list?path=', '/api/terminal/sftp/local/list?path=%2Fetc']);
  });

  it('asks about a bounded number of folders, each once, however many places there are', async () => {
    cwd = '/srv/shell';
    localStorage.setItem('nabiman_sftp_path_local', '/panel');
    const look = { source: local, sessionId: 'sess-1', promptDir: '/srv/prompt', startDir: '/start' };
    const many = ['a/nope.md', 'b/nope.md', 'nope.md', 'x/nope.md'].map((p) => match(p, 'relative'));
    expect(await resolveFile(many, look)).toBeNull();
    expect(lists().length).toBeLessThanOrEqual(12);
    expect(new Set(lists()).size).toBe(lists().length);
  });

  it('does not ask this machine where the shell is for another server\'s files', async () => {
    cwd = '/srv/shell';
    const remote = accountSource('srv-7');
    expect(await resolveFile([match('shell.txt', 'name')], { source: remote, sessionId: 'plain-2' })).toBeNull();
    expect(asked.some((u) => u.endsWith('/cwd'))).toBe(false);
    // The prompt works there all the same.
    expect(await resolveFile([match('prompt.txt', 'name')], { source: remote, sessionId: 'plain-2', promptDir: '/srv/prompt' }))
      .toMatchObject({ path: '/srv/prompt/prompt.txt' });
    expect(asked).toContain('/api/terminal/sftp/srv-7/list?path=%2Fsrv%2Fprompt');
  });

  it('tries the longest reading first and falls back to the shorter', async () => {
    const found = await resolveFile([match('/work/proj/report.pdfand', 'absolute'), match('/work/proj/report.pdf', 'absolute')], { source: local });
    expect(found?.path).toBe('/work/proj/report.pdf');
    expect(lists()).toHaveLength(1);
  });

  it('never leaves the folder tree by .., and does not ask Claude for a share visitor', async () => {
    expect(normalize('/a/./b/../c//d/')).toBe('/a/c/d');
    expect(normalize('/../../etc/passwd')).toBe('/etc/passwd');
    expect(normalize('relative/x')).toBeNull();
    const visitor = shareSource('tok', 'key', false);
    const fetchMock = jest.spyOn(window, 'fetch').mockResolvedValue({ json: async () => ({ success: false, data: null, message: '' }), status: 404 } as Response);
    expect(await resolveFile([match('x.txt', 'name')], { source: visitor, sessionId: 's' })).toBeNull();
    expect(asked).toEqual([]);
    expect(fetchMock.mock.calls.every(([url]) => String(url).startsWith('/api/share/tok/files/list?'))).toBe(true);
    fetchMock.mockRestore();
  });

  it('falls back to the full path printed elsewhere when no folder has the name', async () => {
    const look = { source: accountSource('local'), mentioned: (name: string) => (name === 'app.ts' ? ['/nowhere/app.ts', '/work/proj/src/app.ts'] : []) };
    const found = await resolveFile([match('app.ts', 'name')], look);
    expect(found?.path).toBe('/work/proj/src/app.ts');
    // A name its folders do have never gets that far.
    const asks = jest.fn(() => ['/work/proj/src/app.ts']);
    expect((await resolveFile([match('notes.md', 'name')], { source: accountSource('local'), mentioned: asks }))?.path).toBe('/home/nabi/notes.md');
    expect(asks).not.toHaveBeenCalled();
  });
});

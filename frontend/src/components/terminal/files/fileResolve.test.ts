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
};
const match = (path: string, kind: PathMatch['kind']): PathMatch => ({ start: 0, end: path.length, path, kind });

describe('resolving a printed path', () => {
  let asked: string[];
  beforeEach(() => {
    forgetFiles();
    localStorage.clear();
    asked = [];
    apiRequest.mockImplementation(async (url: string) => {
      asked.push(url);
      if (url.endsWith('/claude')) return { success: true, data: { claude: [{ cwd: '/work/proj' }] } };
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
});

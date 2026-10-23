import { renderHook, act } from '@testing-library/react';
import { accountSource, shareSource, SftpEntry } from './sftpApi';
import { fileType, ownerGroup, sortEntries, template, useColumns } from './sftpColumns';

const e = (p: Partial<SftpEntry>): SftpEntry => ({ name: 'x', kind: 'file', to_dir: false, ...p } as SftpEntry);

describe('new columns', () => {
  it('file type is folder, link, extension, or plain file', () => {
    expect(fileType(e({ name: 'logs', kind: 'dir' }))).toEqual({ label: 'sftp.type.folder', ext: '' });
    expect(fileType(e({ name: 'cur', kind: 'link' }))).toEqual({ label: 'sftp.type.link', ext: '' });
    expect(fileType(e({ name: 'index.PHP' }))).toEqual({ ext: 'php' });
    expect(fileType(e({ name: 'README' }))).toEqual({ label: 'sftp.type.file', ext: '' });
    expect(fileType(e({ name: '.bashrc' }))).toEqual({ label: 'sftp.type.file', ext: '' });
  });
  it('owner/group is one value', () => {
    expect(ownerGroup(e({ owner: 'web', group: 'www' }))).toBe('web / www');
    expect(ownerGroup(e({ uid: 1000, gid: 1000 }))).toBe('1000 / 1000');
    expect(ownerGroup(e({}))).toBe('');
  });
  it('sorts by type (extension) and by owner/group', () => {
    const files = [e({ name: 'b.txt' }), e({ name: 'a.zip' }), e({ name: 'c.php' })];
    expect(sortEntries(files, { key: 'type', desc: false }).map((x) => x.name)).toEqual(['c.php', 'b.txt', 'a.zip']);
    const who = [e({ name: 'a', owner: 'z' }), e({ name: 'b', owner: 'a' })];
    expect(sortEntries(who, { key: 'ownergroup', desc: false }).map((x) => x.name)).toEqual(['b', 'a']);
  });
});

describe('useColumns persistence and reorder', () => {
  beforeEach(() => { localStorage.clear(); });
  it('a share source keeps the layout in the browser, per scope', () => {
    const src = shareSource('t'.repeat(64), 'key', true);
    const { result } = renderHook(() => useColumns(src));
    act(() => result.current.toggle('mode'));       // add
    act(() => result.current.toggle('size'));       // remove (was default)
    act(() => result.current.move('mode', -1));     // reorder
    const saved = JSON.parse(localStorage.getItem(`nabiman_sftp_columns_${src.key}`) || '{}');
    expect(saved.shown).toEqual(result.current.state.shown);
    expect(result.current.state.shown).toContain('mode');
    expect(result.current.state.shown).not.toContain('size');
  });
  it('move swaps neighbours and clamps at the ends', () => {
    const { result } = renderHook(() => useColumns(shareSource('u'.repeat(64), 'k', true)));
    // default shown: size, type, mtime
    act(() => result.current.move('type', -1));
    expect(result.current.state.shown).toEqual(['type', 'size', 'mtime']);
    act(() => result.current.move('type', -1)); // already first: no change
    expect(result.current.state.shown).toEqual(['type', 'size', 'mtime']);
    act(() => result.current.move('mtime', 1)); // already last: no change
    expect(result.current.state.shown).toEqual(['type', 'size', 'mtime']);
  });
  it('template lists icon, name, then shown columns in order', () => {
    const { result } = renderHook(() => useColumns(shareSource('v'.repeat(64), 'k', true)));
    expect(template(result.current.state).startsWith('20px minmax(120px, 1fr)')).toBe(true);
  });
  it('an account source saves to the server, not the browser', () => {
    const calls: Array<{ path: string; init?: { method?: string; body?: string } }> = [];
    jest.spyOn(require('../../../shared/api/useApi'), 'apiRequest')
      .mockImplementation(((path: string, init?: { method?: string; body?: string }) => {
        calls.push({ path, init });
        return Promise.resolve({ success: true, data: init?.method === 'PUT' ? 'saved' : {}, message: '' });
      }) as unknown as (...a: unknown[]) => unknown);
    jest.useFakeTimers();
    const { result } = renderHook(() => useColumns(accountSource('srv_9')));
    act(() => result.current.toggle('mode'));
    act(() => jest.advanceTimersByTime(600));
    expect(localStorage.getItem('nabiman_sftp_columns_srv_9')).toBeNull();
    const put = calls.find((c) => c.init?.method === 'PUT' && c.path === '/api/terminal/sftp/columns');
    expect(put).toBeTruthy();
    const body = JSON.parse(put!.init!.body!);
    expect(body.scope).toBe('srv_9');
    expect(body.layout.shown).toContain('mode');
    jest.useRealTimers();
  });
});

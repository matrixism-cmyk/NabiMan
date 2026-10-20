import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../../i18n';
import AttrsDialog from './AttrsDialog';
import { changeAttrs, modeString, SftpAttrsCaps, SftpEntry } from './sftpApi';

jest.mock('./sftpApi', () => ({ ...jest.requireActual('./sftpApi'), changeAttrs: jest.fn() }));
const change = changeAttrs as jest.Mock;
const source = { base: '/api/terminal/sftp/srv', key: 'srv', writable: true };
const caps: SftpAttrsCaps = { mode: true, owner: true, users: ['root', 'web'], groups: ['www'] };
const entry = (name: string, mode: number, kind: SftpEntry['kind'] = 'file'): SftpEntry =>
  ({ name, kind, to_dir: false, size: 1, mtime: null, mode, uid: 1001, gid: 33, owner: 'web', group: 'www' });

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  change.mockReset();
  change.mockResolvedValue({ success: true, data: { changed: 1, failed: [], failed_count: 0, links_skipped: 0 } });
});

const ui = (targets: SftpEntry[], props: Partial<React.ComponentProps<typeof AttrsDialog>> = {}) => render(
  <I18nProvider><AttrsDialog source={source} caps={caps} onDone={jest.fn()} onClose={jest.fn()}
    targets={targets.map((e) => ({ entry: e, path: `/srv/${e.name}` }))} {...props} /></I18nProvider>);

test('modes read as ls shows them', () => {
  expect(modeString(0o755)).toBe('rwxr-xr-x');
  expect(modeString(0o4755)).toBe('rwsr-xr-x');
  expect(modeString(0o1777)).toBe('rwxrwxrwt');
  expect(modeString(0o2640)).toBe('rw-r-S---');
});

test('one file: ticking a box changes the octal value, and only the mode is sent', async () => {
  const done = jest.fn();
  ui([entry('a.sh', 0o644)], { onDone: done });
  const octal = screen.getByRole('textbox', { name: '권한 (8진수)' });
  expect(octal).toHaveValue('644');
  fireEvent.click(screen.getByRole('checkbox', { name: '소유자 실행' }));
  expect(octal).toHaveValue('744');
  fireEvent.click(screen.getByRole('button', { name: '적용' }));
  await waitFor(() => expect(done).toHaveBeenCalled());
  expect(change).toHaveBeenCalledWith(source, { paths: ['/srv/a.sh'], mode: '744', owner: undefined, group: undefined, recursive: false });
});

test('several with different modes: the mode is left alone unless asked, the owner is set for all', async () => {
  ui([entry('a', 0o644), entry('b', 0o600), entry('dir', 0o755, 'dir')]);
  expect(screen.getByRole('checkbox', { name: '권한 바꾸기' })).not.toBeChecked();
  fireEvent.change(screen.getByRole('combobox', { name: '소유자 (사용자)' }), { target: { value: 'root' } });
  fireEvent.click(screen.getByRole('checkbox', { name: /폴더 안의 모든 항목에도 적용/ }));
  fireEvent.click(screen.getByRole('button', { name: '적용' }));
  await waitFor(() => expect(change).toHaveBeenCalled());
  expect(change.mock.calls[0][1]).toEqual({ paths: ['/srv/a', '/srv/b', '/srv/dir'], mode: undefined, owner: 'root', group: undefined, recursive: true });
});

test('nothing chosen is refused before asking the server', () => {
  ui([entry('a', 0o644), entry('b', 0o600)]);
  fireEvent.click(screen.getByRole('button', { name: '적용' }));
  expect(screen.getByRole('alert')).toHaveTextContent('바꿀 내용이 없습니다');
  expect(change).not.toHaveBeenCalled();
});

test('over FTP there is no owner to change', () => {
  ui([entry('a', 0o644)], { caps: { ...caps, owner: false } });
  expect(screen.queryByRole('combobox', { name: '소유자 (사용자)' })).not.toBeInTheDocument();
  expect(screen.getByText(/FTP로는 소유자와 그룹을 바꿀 수 없어/)).toBeInTheDocument();
});

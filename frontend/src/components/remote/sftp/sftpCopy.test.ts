import { apiPost } from '../../../shared/api/useApi';
import { confirmDialog } from '../../../shared/ui/ConfirmDialog';
import { askAndCopy } from './sftpCopy';
import { droppedRows, SFTP_MIME } from './sftpDrop';

jest.mock('../../../shared/api/useApi', () => ({ apiPost: jest.fn(), apiRequest: jest.fn() }));
jest.mock('../../../shared/ui/ConfirmDialog', () => ({ confirmDialog: jest.fn() }));
const post = apiPost as jest.Mock;
const ask = confirmDialog as jest.Mock;
const t = (key: string) => key;
const rows = { base: '/api/terminal/sftp/a', scope: 'a', name: 'A', paths: ['/srv/x.txt', '/srv/logs'] };
const to = { scope: 'b', name: 'B' };
const copy = { id: 'c1', names: ['x.txt', 'logs'] };

beforeEach(() => { post.mockReset(); ask.mockReset(); });

test('asks, then copies the dropped rows into the folder', async () => {
  ask.mockResolvedValue(true);
  post.mockResolvedValue({ success: true, data: copy });
  expect(await askAndCopy(rows, to, '/home', t)).toEqual({ copy });
  expect(ask.mock.calls[0][0].items).toEqual(['x.txt', 'logs']);
  expect(post).toHaveBeenCalledWith('/api/terminal/copies', { from: 'a', paths: rows.paths, to: 'b', dir: '/home' });
});

test('names already there are asked about once more, and replaced only on yes', async () => {
  ask.mockResolvedValueOnce(true).mockResolvedValueOnce(true);
  post.mockResolvedValueOnce({ success: false, data: { conflicts: ['x.txt'] }, message: 'there' })
    .mockResolvedValueOnce({ success: true, data: copy });
  expect(await askAndCopy(rows, to, '/home', t)).toEqual({ copy });
  expect(ask.mock.calls[1][0]).toMatchObject({ items: ['x.txt'], danger: true });
  expect(post.mock.calls[1][1]).toMatchObject({ overwrite: true });

  ask.mockReset().mockResolvedValueOnce(true).mockResolvedValueOnce(false);
  post.mockReset().mockResolvedValueOnce({ success: false, data: { conflicts: ['x.txt'] } });
  expect(await askAndCopy(rows, to, '/home', t)).toEqual({});
  expect(post).toHaveBeenCalledTimes(1);
});

test('saying no starts nothing, and a refusal comes back as an error', async () => {
  ask.mockResolvedValue(false);
  expect(await askAndCopy(rows, to, '/home', t)).toEqual({});
  expect(post).not.toHaveBeenCalled();
  ask.mockResolvedValue(true);
  post.mockResolvedValue({ success: false, data: null, message: 'Saved server not found' });
  expect(await askAndCopy(rows, to, '/home', t)).toEqual({ error: 'Saved server not found' });
});

test('dropped rows say where they came from; the older list of paths still reads', () => {
  const dt = (value: unknown) => ({ getData: (type: string) => (type === SFTP_MIME ? JSON.stringify(value) : '') }) as unknown as DataTransfer;
  expect(droppedRows(dt(rows))).toEqual(rows);
  expect(droppedRows(dt(['/a', 3, '/b']))).toEqual({ base: undefined, scope: undefined, name: undefined, paths: ['/a', '/b'] });
  expect(droppedRows({ getData: () => 'not json' } as unknown as DataTransfer)).toEqual({ paths: [] });
});

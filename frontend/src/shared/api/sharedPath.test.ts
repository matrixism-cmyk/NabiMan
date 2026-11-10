import { clearToken, fetchWithRefresh, setRefreshToken, setToken } from './useApi';
import { uploadPastedFile } from '../../components/terminal/pasteFile';
import { loadCast } from '../../components/remote/recordingsApi';
import { safeHref } from '../../components/terminal/dictation/VoiceLockField';

/**
 * Calls that used to read the token and call fetch themselves now go through
 * the shared helper, so an access token that expired mid-session is renewed
 * and the request sent again instead of failing with a 401.
 */
const fetchMock = jest.fn();
function response(status: number, body: unknown = {}) {
  return { status, ok: status >= 200 && status < 300, json: async () => body, text: async () => String(body), headers: new Headers() } as unknown as Response;
}
const auth = (options: RequestInit) => (options.headers as Record<string, string>).Authorization;

beforeEach(() => {
  clearToken(); sessionStorage.clear();
  setToken('expired-access'); setRefreshToken('refresh-one');
  fetchMock.mockReset(); global.fetch = fetchMock;
  fetchMock.mockImplementation((url: string, options: RequestInit) => {
    if (url.endsWith('/api/auth/refresh')) {
      return Promise.resolve(response(200, { success: true, data: { token: 'fresh-access', refresh_token: 'refresh-two' } }));
    }
    if (auth(options) !== 'Bearer fresh-access') return Promise.resolve(response(401));
    if (url.endsWith('/api/terminal/paste-file')) return Promise.resolve(response(200, { success: true, data: '/tmp/paste/1.png', message: '' }));
    return Promise.resolve(response(200, '{"cols":100,"rows":30}\n[5,"aGk="]'));
  });
});

test('a pasted image is uploaded again with the renewed token after a 401', async () => {
  const blob = new Blob(['png'], { type: 'image/png' });
  await expect(uploadPastedFile(blob)).resolves.toBe('/tmp/paste/1.png');
  const uploads = fetchMock.mock.calls.filter(([url]) => String(url).endsWith('/api/terminal/paste-file'));
  expect(uploads).toHaveLength(2);
  expect(auth(uploads[1][1])).toBe('Bearer fresh-access');
  // The file's own type survives the shared helper's JSON default.
  expect((uploads[1][1].headers as Record<string, string>)['Content-Type']).toBe('application/octet-stream');
  expect(uploads[1][1].body).toBe(blob);
  expect(sessionStorage.getItem('nabiman_refresh')).toBe('refresh-two');
});

test('a recording loads after the token is renewed', async () => {
  const cast = await loadCast('abc');
  expect(cast).toMatchObject({ cols: 100, rows: 30, duration: 5 });
  expect(fetchMock.mock.calls.filter(([url]) => String(url).endsWith('/api/auth/refresh'))).toHaveLength(1);
});

test('a FormData body keeps the multipart type the browser gives it', async () => {
  setToken('fresh-access');
  const body = new FormData();
  body.append('file', new Blob(['x']), 'x.txt');
  await fetchWithRefresh('/api/upload', { method: 'POST', body });
  const headers = fetchMock.mock.calls[0][1].headers as Record<string, string>;
  expect(headers['Content-Type']).toBeUndefined();
  expect(headers.Authorization).toBe('Bearer fresh-access');
});

test('a server-supplied enrolment link becomes a link only when it is http(s)', () => {
  expect(safeHref('https://v.example/account')).toBe('https://v.example/account');
  expect(safeHref('javascript:alert(1)')).toBeNull();
  expect(safeHref('data:text/html,hi')).toBeNull();
  expect(safeHref('')).toBeNull();
});

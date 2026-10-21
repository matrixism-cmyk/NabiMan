import { waitFor } from '@testing-library/react';
import { apiRequest, clearToken, fetchWithRefresh, setRefreshToken, setToken } from './useApi';

const fetchMock = jest.fn();
function response(status: number, body: unknown = {}) {
  return { status, ok: status >= 200 && status < 300, json: async () => body } as Response;
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
function login(access = 'access-one', refresh = 'refresh-one') {
  setToken(access); setRefreshToken(refresh);
}
const renewed = () => response(200, { success: true, data: { token: 'renewed-one', refresh_token: 'rotated-one' } });
beforeEach(() => {
  clearToken(); sessionStorage.clear(); login(); fetchMock.mockReset(); global.fetch = fetchMock;
});

it('shares renewal within one session and preserves each mutation body and idempotency key', async () => {
  const renewal = deferred<Response>();
  fetchMock.mockImplementation((url: string, options: RequestInit) => {
    if (url === '/api/auth/refresh') return renewal.promise;
    return Promise.resolve(response((options.headers as Record<string, string>).Authorization === 'Bearer renewed-one' ? 200 : 401));
  });
  const requests = ['one', 'two'].map(key => fetchWithRefresh('/api/services/action', {
    method: 'POST', headers: { 'Idempotency-Key': key }, body: JSON.stringify({ key }),
  }));
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(3));
  renewal.resolve(renewed()); await Promise.all(requests);
  expect(fetchMock.mock.calls.filter(([url]) => url === '/api/auth/refresh')).toHaveLength(1);
  for (const key of ['one', 'two']) {
    const calls = fetchMock.mock.calls.filter(([url, options]) => url === '/api/services/action' && options.headers['Idempotency-Key'] === key);
    expect(calls).toHaveLength(2);
    expect(calls[1][1]).toMatchObject({ method: 'POST', body: JSON.stringify({ key }), headers: { Authorization: 'Bearer renewed-one' } });
  }
});

it('never refreshes or replays an old 401 under a replacement account', async () => {
  const old = deferred<Response>(); fetchMock.mockReturnValueOnce(old.promise);
  const pending = fetchWithRefresh('/api/services/action', { method: 'POST', body: '{}' });
  const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  clearToken(); login('access-two', 'refresh-two'); old.resolve(response(401));
  await rejected;
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(sessionStorage.getItem('nabiman_token')).toBe('access-two');
  expect(sessionStorage.getItem('nabiman_refresh')).toBe('refresh-two');
});

it('does not resurrect a logged-out session from a late renewal', async () => {
  const renewal = deferred<Response>();
  fetchMock.mockResolvedValueOnce(response(401)).mockReturnValueOnce(renewal.promise);
  const pending = fetchWithRefresh('/private');
  const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
  clearToken(); renewal.resolve(renewed()); await rejected;
  expect(sessionStorage.getItem('nabiman_token')).toBeNull();
  expect(sessionStorage.getItem('nabiman_refresh')).toBeNull();
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

it('keeps the new account renewal independent of an old failed renewal', async () => {
  const old = deferred<Response>(); const next = deferred<Response>();
  fetchMock.mockResolvedValueOnce(response(401)).mockReturnValueOnce(old.promise)
    .mockResolvedValueOnce(response(401)).mockReturnValueOnce(next.promise).mockResolvedValueOnce(response(200));
  const first = fetchWithRefresh('/old');
  const rejected = expect(first).rejects.toMatchObject({ name: 'AbortError' });
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
  login('access-two', 'refresh-two'); const second = fetchWithRefresh('/new');
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(4));
  old.resolve(response(401)); await rejected;
  expect(sessionStorage.getItem('nabiman_token')).toBe('access-two');
  next.resolve(response(200, { success: true, data: { token: 'renewed-two', refresh_token: 'rotated-two' } }));
  await expect(second).resolves.toMatchObject({ status: 200 });
  expect(fetchMock.mock.calls[3][1].body).toBe(JSON.stringify({ refresh_token: 'refresh-two' }));
  expect(fetchMock.mock.calls[4][1].headers.Authorization).toBe('Bearer renewed-two');
});

it('discards a renewal whose JSON finishes after an account switch', async () => {
  const body = deferred<unknown>();
  const json = jest.fn(() => body.promise);
  fetchMock.mockResolvedValueOnce(response(401)).mockResolvedValueOnce({ ...response(200), json });
  const pending = fetchWithRefresh('/private');
  const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  await waitFor(() => expect(json).toHaveBeenCalled());
  login('access-two', 'refresh-two'); body.resolve({ success: true, data: { token: 'stale-token' } });
  await rejected;
  expect(sessionStorage.getItem('nabiman_token')).toBe('access-two');
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

it('discards an API response whose JSON finishes after an account switch', async () => {
  const body = deferred<unknown>(); const json = jest.fn(() => body.promise);
  fetchMock.mockResolvedValueOnce({ ...response(200), json });
  const pending = apiRequest('/private');
  const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  await waitFor(() => expect(json).toHaveBeenCalled());
  login('access-two', 'refresh-two'); body.resolve({ success: true, data: { secret: 'old-account-data' } });
  await rejected;
});

it('rejects a previous login even when token strings are reused', async () => {
  const old = deferred<Response>(); fetchMock.mockReturnValueOnce(old.promise);
  const pending = fetchWithRefresh('/private');
  const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  clearToken(); login(); old.resolve(response(200)); await rejected;
});

it('uses the already renewed same-session token for a delayed 401', async () => {
  const late = deferred<Response>();
  fetchMock.mockReturnValueOnce(late.promise).mockResolvedValueOnce(response(401))
    .mockResolvedValueOnce(renewed()).mockResolvedValueOnce(response(200)).mockResolvedValueOnce(response(200));
  const delayed = fetchWithRefresh('/late'); await fetchWithRefresh('/fast');
  late.resolve(response(401)); await delayed;
  expect(fetchMock.mock.calls.filter(([url]) => url === '/api/auth/refresh')).toHaveLength(1);
  expect(fetchMock.mock.calls[4][1].headers.Authorization).toBe('Bearer renewed-one');
});

it('does not renew or replay an aborted request', async () => {
  const old = deferred<Response>(); fetchMock.mockReturnValueOnce(old.promise);
  const controller = new AbortController();
  const pending = fetchWithRefresh('/private', { signal: controller.signal });
  const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
  controller.abort(); old.resolve(response(401)); await rejected;
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

it('retries at most once when the renewed access token is also rejected', async () => {
  fetchMock.mockResolvedValueOnce(response(401)).mockResolvedValueOnce(renewed()).mockResolvedValueOnce(response(401));
  await expect(fetchWithRefresh('/private')).resolves.toMatchObject({ status: 401 });
  expect(fetchMock).toHaveBeenCalledTimes(3);
});

it('removes the previous refresh credential when a new login has no refresh token', async () => {
  setToken('access-two'); expect(sessionStorage.getItem('nabiman_refresh')).toBeNull();
  fetchMock.mockResolvedValueOnce(response(401));
  await expect(fetchWithRefresh('/private')).resolves.toMatchObject({ status: 401 });
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

it('uses the rotated refresh credential on the next renewal without changing account identity', async () => {
  fetchMock.mockResolvedValueOnce(response(401)).mockResolvedValueOnce(renewed()).mockResolvedValueOnce(response(200))
    .mockResolvedValueOnce(response(401)).mockResolvedValueOnce(response(200, { success: true,
      data: { token: 'renewed-twice', refresh_token: 'rotated-twice' } })).mockResolvedValueOnce(response(200));
  await fetchWithRefresh('/first');
  expect(sessionStorage.getItem('nabiman_refresh')).toBe('rotated-one');
  await fetchWithRefresh('/second');
  expect(fetchMock.mock.calls[4][1].body).toBe(JSON.stringify({ refresh_token: 'rotated-one' }));
  expect(sessionStorage.getItem('nabiman_refresh')).toBe('rotated-twice');
});

it('a delayed 401 shares rotation even when access token bytes did not change', async () => {
  const late = deferred<Response>();
  fetchMock.mockReturnValueOnce(late.promise).mockResolvedValueOnce(response(401))
    .mockResolvedValueOnce(response(200, { success: true, data: { token: 'access-one', refresh_token: 'rotated-one' } }))
    .mockResolvedValueOnce(response(200)).mockResolvedValueOnce(response(200));
  const delayed = fetchWithRefresh('/late'); await fetchWithRefresh('/fast');
  late.resolve(response(401)); await delayed;
  expect(fetchMock.mock.calls.filter(([url]) => url === '/api/auth/refresh')).toHaveLength(1);
  expect(sessionStorage.getItem('nabiman_refresh')).toBe('rotated-one');
});

function requiredMfa() {
  return { ...response(403), headers: new Headers({ 'X-Nabiman-MFA-Required': '1' }) } as Response;
}
it('signals an active MFA policy denial without refreshing or removing enrollment credentials', async () => {
  const changed = jest.fn(); window.addEventListener('nabiman_mfa_required', changed);
  try {
    fetchMock.mockResolvedValueOnce(requiredMfa());
    expect((await fetchWithRefresh('/api/remote-servers')).status).toBe(403);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(sessionStorage.getItem('nabiman_token')).toBe('access-one');
  } finally { window.removeEventListener('nabiman_mfa_required', changed); }
});
it('does not gate a replacement account from a delayed policy denial', async () => {
  const changed = jest.fn(); window.addEventListener('nabiman_mfa_required', changed);
  try {
    const old = deferred<Response>(); fetchMock.mockReturnValueOnce(old.promise);
    const pending = fetchWithRefresh('/api/remote-servers');
    const rejected = expect(pending).rejects.toMatchObject({ name: 'AbortError' });
    login('new-access','new-refresh'); old.resolve(requiredMfa()); await rejected;
    expect(changed).not.toHaveBeenCalled();
  } finally { window.removeEventListener('nabiman_mfa_required', changed); }
});

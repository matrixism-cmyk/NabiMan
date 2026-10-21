import { issueShareTicket, readShareInfo, ShareAdmissionError } from './shareAdmission';
const mockedFetch = jest.fn();
beforeEach(() => { global.fetch = mockedFetch; mockedFetch.mockReset(); sessionStorage.setItem('nabiman_token', 'control-plane-secret'); });
const ticket = 'a'.repeat(64);
const reply = (data: unknown, status = 200) => new Response(JSON.stringify(data), { status });

it('sends password and query only in the body without account headers, cookies or persistent proof', async () => {
  mockedFetch.mockResolvedValue(reply({ success: true, data: { ticket, expires_in_seconds: 30 } }));
  expect(await issueShareTicket('link', 'memory-only-password', 'cols=80')).toEqual({ ticket, expires_in_seconds: 30 });
  expect(mockedFetch).toHaveBeenCalledWith('/api/share/link/auth', expect.objectContaining({ credentials: 'omit', cache: 'no-store', method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ password: 'memory-only-password', query: 'cols=80' }) }));
  expect(sessionStorage.getItem('nabiman_token')).toBe('control-plane-secret');
  expect(JSON.stringify(localStorage)).not.toContain(ticket);
  expect(JSON.stringify(sessionStorage)).not.toContain('memory-only-password');
});
it.each([undefined, 0, 31, NaN])('rejects missing or invalid ticket lifetimes: %s', async expires => {
  mockedFetch.mockResolvedValue(reply({ success: true, data: { ticket, expires_in_seconds: expires } }));
  await expect(issueShareTicket('link', '', '')).rejects.toBeInstanceOf(ShareAdmissionError);
});
it('rejects the previous reusable signed proof format', async () => {
  mockedFetch.mockResolvedValue(reply({ success: true, data: { ticket: '123456789.signature', expires_in_seconds: 30 } }));
  await expect(issueShareTicket('link', '', '')).rejects.toBeInstanceOf(ShareAdmissionError);
});
it.each([[429, 'share.authLimited'], [403, 'share.authDenied'], [503, 'share.authUnavailable']])('reports status %s without an account token refresh', async (status, key) => {
  mockedFetch.mockResolvedValue(reply({ success: false }, Number(status)));
  await expect(issueShareTicket('link', 'wrong', '')).rejects.toMatchObject({ translationKey: key });
  expect(mockedFetch).toHaveBeenCalledTimes(1);
});
it('rejects malformed metadata and propagates cancellation', async () => {
  mockedFetch.mockResolvedValue(reply({ success: true, data: {} }));
  await expect(readShareInfo('link')).rejects.toBeInstanceOf(ShareAdmissionError);
  const controller = new AbortController(); controller.abort();
  mockedFetch.mockResolvedValue(reply({ success: true, data: { ticket, expires_in_seconds: 30 } }));
  await expect(issueShareTicket('link', '', '', controller.signal)).rejects.toMatchObject({ name: 'AbortError' });
});
it('takes how dictation ends what was said from the server, with safe defaults', async () => {
  const info = { label: 'x', needs_password: false, read_only: false, alive: true, expires_unix: 9, cols: 80, rows: 24, dictation: true };
  mockedFetch.mockResolvedValue(reply({ success: true, data: { ...info, dictation_pause_ms: 2000, dictation_auto_enter: true } }));
  expect(await readShareInfo('link')).toMatchObject({ dictation_pause_ms: 2000, dictation_auto_enter: true });
  mockedFetch.mockResolvedValue(reply({ success: true, data: { ...info, dictation_pause_ms: 'soon' } }));
  expect(await readShareInfo('link')).toMatchObject({ dictation_pause_ms: 1500, dictation_auto_enter: false });
});

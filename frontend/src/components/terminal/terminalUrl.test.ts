import { buildTerminalUrl, ticketedTerminalUrl, ticketedShareUrl } from './terminalUrl';
import { apiRequest } from '../../shared/api/useApi';
import { DEFAULT_TERMINAL_SETTINGS } from './settings';
jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
const ticket = 'a'.repeat(64);
beforeEach(() => { jest.clearAllMocks(); sessionStorage.setItem('nabiman_token', 'never-in-a-url'); });
it('builds account target and session parameters without embedding the login token', () => {
  const url = buildTerminalUrl({ settings: DEFAULT_TERMINAL_SETTINGS, target: { kind: 'ssh', serverId: 'server-1' }, sessionId: 'session-1', join: true, cols: 80, rows: 24 });
  expect(url).not.toContain('token=');
  expect(url).not.toContain('never-in-a-url');
  expect(url).toContain('server_id=server-1');
  expect(url).toContain('session_id=session-1');
  expect(url).toContain('join=1');
});
it('binds exact query bytes and gets a fresh ticket on every connection', async () => {
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: { ticket, expires_in_seconds: 30 } });
  const url = `ws://${window.location.host}/api/terminal?session_id=encoded%20value&history=1`;
  expect(await ticketedTerminalUrl(url)).toBe(url + '&ticket=' + ticket);
  expect(await ticketedTerminalUrl(url)).toBe(url + '&ticket=' + ticket);
  expect(apiRequest).toHaveBeenCalledTimes(2);
  expect(apiRequest).toHaveBeenCalledWith('/api/terminal/tickets', expect.objectContaining({ method: 'POST', body: JSON.stringify({ query: 'session_id=encoded%20value&history=1' }) }));
});
it('rejects another origin, JWT query, and forged admission response before opening a socket', async () => {
  await expect(ticketedTerminalUrl('ws://other.example/api/terminal?cols=80')).rejects.toThrow();
  await expect(ticketedTerminalUrl(`ws://${window.location.host}/api/terminal?token=secret`)).rejects.toThrow();
  expect(apiRequest).not.toHaveBeenCalled();
  (apiRequest as jest.Mock).mockResolvedValue({ success: false, message: 'Forbidden' });
  await expect(ticketedTerminalUrl(`ws://${window.location.host}/api/terminal?cols=80`)).rejects.toThrow('Forbidden');
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: { ticket: 'forged', expires_in_seconds: 30 } });
  await expect(ticketedTerminalUrl(`ws://${window.location.host}/api/terminal?cols=80`)).rejects.toThrow('Invalid terminal connection ticket');
});
it('keeps anonymous share admission independent of account tickets', () => {
  const url = buildTerminalUrl({ settings: DEFAULT_TERMINAL_SETTINGS, target: { kind: 'local' }, share: { token: 'share-link' }, sessionId: '', join: false, cols: 80, rows: 24 });
  expect(url).toContain('/api/share/share-link/ws');
  expect(url).not.toContain('ticket=');
  expect(url).not.toContain('never-in-a-url');
});

it('obtains separate one-use share admission on every reconnect and binds exact geometry', async () => {
  const acquireTicket = jest.fn().mockResolvedValueOnce('a'.repeat(64)).mockResolvedValueOnce('b'.repeat(64));
  const url = `ws://${window.location.host}/api/share/capability/ws?cols=80&rows=24`;
  expect(await ticketedShareUrl(url, { token: 'capability', acquireTicket })).toBe(url + '&ticket=' + 'a'.repeat(64));
  expect(await ticketedShareUrl(url, { token: 'capability', acquireTicket })).toBe(url + '&ticket=' + 'b'.repeat(64));
  expect(acquireTicket).toHaveBeenCalledTimes(2);
  expect(acquireTicket).toHaveBeenCalledWith('cols=80&rows=24', undefined);
  expect(apiRequest).not.toHaveBeenCalled();
});
it('rejects cross-origin, another share and embedded reusable credentials', async () => {
  const share = { token: 'one', acquireTicket: jest.fn() };
  for (const url of ['ws://other.example/api/share/one/ws', `ws://${window.location.host}/api/share/two/ws`, `ws://${window.location.host}/api/share/one/ws?ticket=old`, `ws://${window.location.host}/api/share/one/ws?token=jwt`]) await expect(ticketedShareUrl(url, share)).rejects.toThrow();
  expect(share.acquireTicket).not.toHaveBeenCalled();
});

it('starts Claude Code only when opening a new local session, never on rejoin', () => {
  const start = { user: 'nabi', dir: '/workspace/나비 man', skipPermissions: true, resume: true, title: '나비' };
  const fresh = buildTerminalUrl({ settings: DEFAULT_TERMINAL_SETTINGS, target: { kind: 'local', start }, sessionId: '', join: false, cols: 80, rows: 24 });
  expect(fresh).toContain('&start_user=nabi&start_dir=%2Fworkspace%2F%EB%82%98%EB%B9%84%20man');
  expect(fresh).toContain('&start_title=%EB%82%98%EB%B9%84');
  expect(fresh).toContain('&claude_skip=1&claude_continue=1');
  expect(/^[\x21-\x7e]+$/.test(fresh)).toBe(true);
  const rejoin = buildTerminalUrl({ settings: DEFAULT_TERMINAL_SETTINGS, target: { kind: 'local', start }, sessionId: 'pty_1', join: true, cols: 80, rows: 24 });
  expect(rejoin).not.toContain('start_');
  expect(rejoin).not.toContain('claude_');
});

it('opening a background Claude session sends only its card id', () => {
  const start = { user: '', dir: '', skipPermissions: false, resume: false, title: 'n8n', attach: 'lc:fb91fc72-97e3' };
  const url = buildTerminalUrl({ settings: DEFAULT_TERMINAL_SETTINGS, target: { kind: 'local', start }, sessionId: '', join: false, cols: 80, rows: 24 });
  expect(url).toContain('&claude_attach=lc%3Afb91fc72-97e3&start_title=n8n');
  expect(url).not.toContain('start_user');
  expect(url).not.toContain('claude_skip');
});

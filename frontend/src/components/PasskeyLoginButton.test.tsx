import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { setRefreshToken, setToken } from '../shared/api/useApi';
import LoginScreen from './LoginScreen';
import PasskeyLoginButton from './PasskeyLoginButton';

jest.mock('../shared/api/useApi', () => ({ setToken: jest.fn(), setRefreshToken: jest.fn() }));

const answer = (status: number, body: object, headers: Record<string, string> = {}) =>
  ({ status, ok: status < 400, headers: { get: (name: string) => headers[name] || null }, json: async () => body }) as unknown as Response;
const challenge = answer(200, { success: true, data: { challenge_id: 'c1', publicKey: { challenge: 'AQID', rpId: 'man.example.com', userVerification: 'required' } } });
const assertion = { id: 'AQI', rawId: new Uint8Array([1, 2]).buffer, type: 'public-key',
  response: { authenticatorData: new Uint8Array([1]).buffer, clientDataJSON: new Uint8Array([2]).buffer, signature: new Uint8Array([3]).buffer, userHandle: new Uint8Array([9]).buffer } };

let get: jest.Mock;
let fetchMock: jest.Mock;
function browserWithPasskeys() {
  get = jest.fn().mockResolvedValue(assertion);
  Object.defineProperty(window, 'PublicKeyCredential', { configurable: true, value: function PublicKeyCredential() { /* stand-in */ } });
  Object.defineProperty(navigator, 'credentials', { configurable: true, value: { create: jest.fn(), get } });
}
beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  fetchMock = jest.fn();
  (global as unknown as { fetch: jest.Mock }).fetch = fetchMock;
  jest.clearAllMocks();
});
afterEach(() => {
  delete (window as { PublicKeyCredential?: unknown }).PublicKeyCredential;
  delete (navigator as { credentials?: unknown }).credentials;
});

function button(overrides: Partial<React.ComponentProps<typeof PasskeyLoginButton>> = {}) {
  const props = { onLogin: jest.fn(), onError: jest.fn(), limited: jest.fn().mockReturnValue(false), ...overrides };
  render(<I18nProvider><PasskeyLoginButton {...props} /></I18nProvider>);
  return props;
}

test('the login screen offers a passkey only where the browser has them', () => {
  fetchMock.mockResolvedValue(answer(200, { success: true, data: { enabled: false } }));
  const { unmount } = render(<I18nProvider><LoginScreen onLogin={() => undefined} /></I18nProvider>);
  expect(screen.getByRole('button', { name: '로그인' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '패스키로 로그인' })).not.toBeInTheDocument();
  unmount();

  browserWithPasskeys();
  render(<I18nProvider><LoginScreen onLogin={() => undefined} /></I18nProvider>);
  expect(screen.getByRole('button', { name: '패스키로 로그인' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '로그인' })).toBeInTheDocument();
});

test('a passkey sign-in asks for no username, posts the answer and keeps the tokens', async () => {
  browserWithPasskeys();
  fetchMock.mockResolvedValueOnce(challenge)
    .mockResolvedValueOnce(answer(200, { success: true, data: { token: 'access', refresh_token: 'refresh', username: 'kim', role: 'admin' } }));
  const props = button();
  fireEvent.click(screen.getByRole('button', { name: '패스키로 로그인' }));
  await waitFor(() => expect(props.onLogin).toHaveBeenCalled());

  expect(fetchMock.mock.calls[0][0]).toBe('/api/auth/passkeys/login/begin');
  expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({});
  const asked = get.mock.calls[0][0];
  expect(Array.from(asked.publicKey.challenge as Uint8Array)).toEqual([1, 2, 3]);
  expect(asked.publicKey.allowCredentials).toEqual([]);
  expect(asked.mediation).toBeUndefined();
  expect(fetchMock.mock.calls[1][0]).toBe('/api/auth/passkeys/login/finish');
  expect(JSON.parse(fetchMock.mock.calls[1][1].body)).toEqual({
    challenge_id: 'c1',
    credential: { id: 'AQI', rawId: 'AQI', type: 'public-key', response: { authenticatorData: 'AQ', clientDataJSON: 'Ag', signature: 'Aw', userHandle: 'CQ' } },
  });
  expect(setToken).toHaveBeenCalledWith('access');
  expect(setRefreshToken).toHaveBeenCalledWith('refresh');
  expect(props.onError).toHaveBeenLastCalledWith('');
});

test('refusals are told apart: not set up, refused passkey, an account that is off, cancelled, rate limited', async () => {
  browserWithPasskeys();
  const off = '가입 승인 대기, 반려 또는 이용 정지 상태입니다. 최고관리자에게 문의하세요.';
  const cases: [Response[], (() => void) | null, string][] = [
    [[answer(503, { success: false, message: 'Passkeys are not configured on this server' })], null, '이 서버는 아직 패스키를 쓸 수 있게 설정되지 않았습니다. 관리자에게 문의하세요.'],
    [[challenge, answer(401, { success: false, message: 'Invalid credentials' })], null, '이 패스키로는 로그인할 수 없습니다. 삭제된 패스키일 수 있습니다. 비밀번호로 로그인해 주세요.'],
    [[challenge, answer(403, { success: false, message: off })], null, off],
    [[challenge], () => get.mockRejectedValueOnce(new DOMException('no', 'NotAllowedError')), '패스키 확인을 취소했거나 시간이 지났습니다.'],
  ];
  const limited = jest.fn().mockReturnValue(false);
  const props = button({ limited });
  for (const [answers, prepare, said] of cases) {
    fetchMock.mockReset();
    answers.forEach(a => fetchMock.mockResolvedValueOnce(a));
    prepare?.();
    fireEvent.click(screen.getByRole('button', { name: '패스키로 로그인' }));
    await waitFor(() => expect(props.onError).toHaveBeenLastCalledWith(said));
    await screen.findByRole('button', { name: '패스키로 로그인' });
  }
  expect(props.onLogin).not.toHaveBeenCalled();
  expect(setToken).not.toHaveBeenCalled();

  // A 429 goes to the login screen's own countdown, and nothing more is asked.
  fetchMock.mockReset();
  fetchMock.mockResolvedValueOnce(answer(429, {}, { 'Retry-After': '30' }));
  limited.mockImplementation((res: Response) => res.status === 429);
  get.mockClear();
  fireEvent.click(screen.getByRole('button', { name: '패스키로 로그인' }));
  await waitFor(() => expect(limited).toHaveBeenLastCalledWith(expect.objectContaining({ status: 429 })));
  await screen.findByRole('button', { name: '패스키로 로그인' });
  expect(get).not.toHaveBeenCalled();
  expect(fetchMock).toHaveBeenCalledTimes(1);
});

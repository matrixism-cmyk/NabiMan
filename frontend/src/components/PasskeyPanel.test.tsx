import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiPost, useApi } from '../shared/api/useApi';
import PasskeyPanel from './PasskeyPanel';

jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn() }));

const list = { enabled: true, reason: null, local_password_available: true, limit: 10, passkeys: [
  { id: 'k1', name: '회사 노트북', created_at: '2026-11-17 01:02:03', last_used: '2026-11-17 04:05:06' },
  { id: 'k2', name: '휴대폰', created_at: '2026-11-16 01:02:03', last_used: null },
] };
const made = { id: 'AQI', rawId: new Uint8Array([1, 2]).buffer, type: 'public-key',
  response: { attestationObject: new Uint8Array([3]).buffer, clientDataJSON: new Uint8Array([4]).buffer } };
const begun = { success: true, data: { challenge_id: 'c1', publicKey: { challenge: 'AQID', user: { id: 'BAUG', name: 'kim', displayName: 'kim' }, excludeCredentials: [{ type: 'public-key', id: 'BwgJ' }] } } };

let create: jest.Mock;
let refetch: jest.Mock;
function show(data: object = list, supported = true) {
  refetch = jest.fn();
  (useApi as jest.Mock).mockReturnValue({ loading: false, error: null, refetch, data });
  if (!supported) {
    delete (window as { PublicKeyCredential?: unknown }).PublicKeyCredential;
    delete (navigator as { credentials?: unknown }).credentials;
  } else {
    create = jest.fn().mockResolvedValue(made);
    Object.defineProperty(window, 'PublicKeyCredential', { configurable: true, value: function PublicKeyCredential() { /* stand-in */ } });
    Object.defineProperty(navigator, 'credentials', { configurable: true, value: { create, get: jest.fn() } });
  }
  return render(<I18nProvider><PasskeyPanel /></I18nProvider>);
}
beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); jest.clearAllMocks(); });
afterEach(() => {
  delete (window as { PublicKeyCredential?: unknown }).PublicKeyCredential;
  delete (navigator as { credentials?: unknown }).credentials;
});

test('lists each passkey with its name, when it was registered and when it was last used', () => {
  show();
  expect(useApi).toHaveBeenCalledWith('/api/auth/passkeys');
  const rows = screen.getAllByRole('row');
  expect(rows).toHaveLength(3);
  expect(rows[1]).toHaveTextContent('회사 노트북');
  expect(rows[1]).toHaveTextContent('2026-11-17 01:02:03');
  expect(rows[1]).toHaveTextContent('2026-11-17 04:05:06');
  expect(rows[2]).toHaveTextContent('휴대폰');
  expect(rows[2]).toHaveTextContent('아직 사용 안 함');
});

test('registers this device with a name and the current password', async () => {
  show();
  (apiPost as jest.Mock).mockResolvedValueOnce(begun).mockResolvedValueOnce({ success: true, data: 'Passkey registered' });
  fireEvent.change(screen.getByLabelText('패스키 이름'), { target: { value: '집 데스크톱' } });
  fireEvent.change(screen.getByLabelText('현재 비밀번호'), { target: { value: 'my password' } });
  fireEvent.click(screen.getByRole('button', { name: '이 기기를 패스키로 등록' }));
  expect(await screen.findByRole('status')).toHaveTextContent('패스키를 등록했습니다');

  expect(apiPost).toHaveBeenNthCalledWith(1, '/api/auth/passkeys/register/begin', { password: 'my password' });
  const asked = create.mock.calls[0][0].publicKey;
  expect(Array.from(asked.challenge as Uint8Array)).toEqual([1, 2, 3]);
  expect(Array.from(asked.user.id as Uint8Array)).toEqual([4, 5, 6]);
  expect(Array.from(asked.excludeCredentials[0].id as Uint8Array)).toEqual([7, 8, 9]);
  expect(apiPost).toHaveBeenNthCalledWith(2, '/api/auth/passkeys/register/finish', {
    challenge_id: 'c1', name: '집 데스크톱',
    credential: { id: 'AQI', rawId: 'AQI', type: 'public-key', response: { attestationObject: 'Aw', clientDataJSON: 'BA' } },
  });
  expect(refetch).toHaveBeenCalled();
  expect(screen.getByLabelText('현재 비밀번호')).toHaveValue('');
});

test('a wrong password, a cancelled prompt and a device that already has one are each said plainly', async () => {
  show();
  const submit = () => {
    fireEvent.change(screen.getByLabelText('패스키 이름'), { target: { value: 'x' } });
    fireEvent.change(screen.getByLabelText('현재 비밀번호'), { target: { value: 'pw' } });
    fireEvent.click(screen.getByRole('button', { name: '이 기기를 패스키로 등록' }));
  };
  (apiPost as jest.Mock).mockResolvedValueOnce({ success: false, message: 'Invalid password' });
  submit();
  expect(await screen.findByRole('alert')).toHaveTextContent('현재 비밀번호가 맞지 않습니다.');
  expect(create).not.toHaveBeenCalled();

  (apiPost as jest.Mock).mockResolvedValueOnce(begun);
  create.mockRejectedValueOnce(new DOMException('no', 'NotAllowedError'));
  submit();
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('패스키 확인을 취소했거나 시간이 지났습니다.'));

  (apiPost as jest.Mock).mockResolvedValueOnce(begun);
  create.mockRejectedValueOnce(new DOMException('dup', 'InvalidStateError'));
  submit();
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('이 기기에는 이미 이 계정의 패스키가 있습니다.'));
  expect(apiPost).toHaveBeenCalledTimes(3);
  expect(refetch).not.toHaveBeenCalled();
});

test('renames a passkey', async () => {
  show();
  (apiPost as jest.Mock).mockResolvedValueOnce({ success: true, data: 'Passkey renamed' });
  fireEvent.click(screen.getAllByRole('button', { name: '이름 바꾸기' })[1]);
  const input = screen.getByLabelText('패스키 이름');
  expect(input).toHaveValue('휴대폰');
  fireEvent.change(input, { target: { value: '새 휴대폰' } });
  fireEvent.click(screen.getByRole('button', { name: '저장' }));
  expect(await screen.findByRole('status')).toHaveTextContent('이름을 바꿨습니다.');
  expect(apiPost).toHaveBeenCalledWith('/api/auth/passkeys/rename', { id: 'k2', name: '새 휴대폰' });
  expect(refetch).toHaveBeenCalled();
});

test('deleting asks for the current password and reports a refusal', async () => {
  show();
  fireEvent.click(screen.getAllByRole('button', { name: '삭제' })[0]);
  expect(screen.getByText(/"회사 노트북" 패스키를 삭제하려면 현재 비밀번호를 입력하세요/)).toBeInTheDocument();
  expect(apiPost).not.toHaveBeenCalled();

  (apiPost as jest.Mock).mockResolvedValueOnce({ success: false, message: 'Invalid password' });
  fireEvent.change(screen.getByLabelText('현재 비밀번호'), { target: { value: 'wrong' } });
  const confirm = () => fireEvent.click(screen.getAllByRole('button', { name: '삭제' }).find(b => (b as HTMLButtonElement).type === 'submit')!);
  confirm();
  expect(await screen.findByRole('alert')).toHaveTextContent('현재 비밀번호가 맞지 않습니다.');
  expect(refetch).not.toHaveBeenCalled();

  (apiPost as jest.Mock).mockResolvedValueOnce({ success: true, data: 'Passkey deleted' });
  fireEvent.change(screen.getByLabelText('현재 비밀번호'), { target: { value: 'right' } });
  confirm();
  expect(await screen.findByRole('status')).toHaveTextContent('패스키를 삭제했습니다.');
  expect(apiPost).toHaveBeenLastCalledWith('/api/auth/passkeys/delete', { id: 'k1', password: 'right' });
  expect(refetch).toHaveBeenCalled();
});

test('says why nothing can be registered: not set up, no passkeys in this browser, an external account', () => {
  const { unmount } = show({ ...list, enabled: false, reason: 'NABIMAN_PUBLIC_ORIGIN is not set', passkeys: [] });
  expect(screen.getByRole('status')).toHaveTextContent('이 서버는 아직 패스키를 쓸 수 있게 설정되지 않았습니다.');
  expect(screen.getByText(/NABIMAN_PUBLIC_ORIGIN is not set/)).toBeInTheDocument();
  expect(screen.getByText('등록한 패스키가 없습니다.')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '이 기기를 패스키로 등록' })).not.toBeInTheDocument();
  unmount();

  const second = show(list, false);
  expect(screen.getByText('이 브라우저는 패스키를 지원하지 않아 새로 등록할 수 없습니다.')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '이 기기를 패스키로 등록' })).not.toBeInTheDocument();
  expect(screen.getAllByRole('button', { name: '삭제' })).toHaveLength(2);
  second.unmount();

  show({ ...list, local_password_available: false, passkeys: [] });
  expect(screen.getByText(/외부 계정\(LDAP, 통합 로그인\)으로 로그인하는 계정은 패스키를 등록할 수 없습니다/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '이 기기를 패스키로 등록' })).not.toBeInTheDocument();
});

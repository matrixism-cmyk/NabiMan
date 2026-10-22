import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import RegistrationForm from './RegistrationForm';
import UserManagementPanel from './UserManagementPanel';
import { useApi, apiPost, apiRequest } from '../shared/api/useApi';
import { useMecList } from '../hooks/mec/useMecApi';

jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiRequest: jest.fn() }));
jest.mock('../hooks/mec/useMecApi', () => ({ useMecList: jest.fn() }));
const scope = { kind: 'server', scope_id: 's1', role: 'admin' };
const admin = { id: 'root', username: 'admin', role: 'admin', status: 'active', memberships: [], requested_memberships: [] };
const pending = { id: 'new', username: 'new-user', role: 'viewer', status: 'pending', memberships: [], requested_memberships: [] };
const refetch = jest.fn();
function mockData(me: object, users: object[]) {
  (useApi as jest.Mock).mockImplementation((path: string) => ({
    data: path === '/api/users/me' ? me : path === '/api/users' ? users
      : path.endsWith('/overview') ? { own_servers: [], assigned_servers: [], terminal_sessions: [], share_links: [], sign_ins: [] }
      : [{ id: 's1', name: 'Own server' }],
    loading: false, error: null, refetch,
  }));
  (useMecList as jest.Mock).mockReturnValue({ data: [{ id: 't1', display_name: 'Own tenant' }] });
  (apiPost as jest.Mock).mockResolvedValue({ success: true, data: '저장했습니다' });
}
test('signup confirms password and leaves applicant waiting for approval', async () => {
  const fetchMock = jest.spyOn(global, 'fetch').mockResolvedValue({ json: async () => ({ success: true, data: '최고관리자 승인 대기' }) } as Response);
  render(<RegistrationForm onBack={jest.fn()} />);
  fireEvent.change(screen.getByLabelText('가입 아이디'), { target: { value: 'new-user' } });
  fireEvent.change(screen.getByLabelText('가입 비밀번호'), { target: { value: 'long-password-123' } });
  fireEvent.change(screen.getByLabelText('비밀번호 확인'), { target: { value: 'different' } });
  fireEvent.click(screen.getByRole('button', { name: '가입 신청' }));
  expect(screen.getByText('비밀번호 확인이 일치하지 않습니다.')).toBeInTheDocument();
  expect(fetchMock).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText('비밀번호 확인'), { target: { value: 'long-password-123' } });
  fireEvent.click(screen.getByRole('button', { name: '가입 신청' }));
  expect(await screen.findByText('최고관리자 승인 대기')).toBeInTheDocument();
  expect(sessionStorage.getItem('nabiman_token')).toBeNull();
  fetchMock.mockRestore();
});
test('super administrator approves with the selected tenant membership', async () => {
  mockData(admin, [admin, pending]);
  render(<UserManagementPanel />);
  expect(screen.getByRole('heading', { name: '최고관리자 · 회원 승인 및 소속 관리' })).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '가입 신청 확인' }));
  expect(screen.queryByText('admin', { selector: 'strong' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByText('new-user', { selector: 'strong' }));
  fireEvent.change(screen.getByLabelText('소속 추가'), { target: { value: 'tenant:t1' } });
  fireEvent.change(screen.getByLabelText('소속 권한'), { target: { value: 'admin' } });
  fireEvent.click(screen.getByRole('button', { name: '승인하고 저장' }));
  await waitFor(() => expect(apiPost).toHaveBeenCalledWith('/api/users/new/review', {
    status: 'active', memberships: [{ kind: 'tenant', scope_id: 't1', role: 'admin' }],
  }));
});

const remote = { id: 'rm', username: 'remote-user', role: 'remote', status: 'active', memberships: [], requested_memberships: [], totp_enabled: false };
test('super administrator grants a remote-only member 세션 공유 and limits, keeping the rest as it was', async () => {
  mockData(admin, [admin, remote]);
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: '저장했습니다' });
  render(<UserManagementPanel />);
  fireEvent.click(screen.getByText('remote-user', { selector: 'strong' }));
  fireEvent.click(screen.getByRole('tab', { name: '원격 권한' }));
  fireEvent.click(screen.getByLabelText(/세션 공유/));
  fireEvent.click(screen.getByLabelText(/받아쓰기/));
  fireEvent.change(screen.getByLabelText(/동시 세션/), { target: { value: '3' } });
  fireEvent.click(screen.getByRole('button', { name: '원격 권한 저장' }));
  await waitFor(() => expect(apiRequest).toHaveBeenCalledWith('/api/users/rm', expect.objectContaining({ method: 'PUT' })));
  const body = JSON.parse((apiRequest as jest.Mock).mock.calls.find(c => c[0] === '/api/users/rm')[1].body);
  expect(body.remote_permissions).toMatchObject({ share_sessions: true, share_files: false, dictation: false, max_sessions: 3, register_servers: true });
});

test('super administrator limits where and when an account may be used', async () => {
  mockData(admin, [admin, remote]);
  (apiRequest as jest.Mock).mockImplementation(async (path: string) => (path === '/api/users/my-address'
    ? { success: true, data: '203.0.113.5' } : { success: true, data: '저장했습니다' }));
  render(<UserManagementPanel />);
  fireEvent.click(screen.getByText('remote-user', { selector: 'strong' }));
  fireEvent.click(screen.getByRole('tab', { name: '접속 제한' }));
  fireEvent.click(screen.getByLabelText(/2단계 인증 필수/));
  fireEvent.click(await screen.findByRole('button', { name: '추가' }));
  fireEvent.click(screen.getByLabelText(/정해진 시간에만/));
  fireEvent.click(screen.getByRole('button', { name: '접속 제한 저장' }));
  await waitFor(() => expect(apiRequest).toHaveBeenCalledWith('/api/users/rm', expect.objectContaining({ method: 'PUT' })));
  const body = JSON.parse((apiRequest as jest.Mock).mock.calls.find(c => c[0] === '/api/users/rm')[1].body);
  expect(body.access).toEqual({ expires_at: null, require_mfa: true, allowed_networks: ['203.0.113.5'], allowed_hours: { start: 9, end: 18, days: [0, 1, 2, 3, 4] } });
});
test('scope manager sees delegation controls without signup approval or admin promotion', () => {
  const manager = { ...admin, id: 'manager', username: 'manager', role: 'viewer', memberships: [scope] };
  const member = { ...pending, status: 'active', memberships: [{ ...scope, role: 'viewer' }] };
  mockData(manager, [manager, member]);
  render(<UserManagementPanel />);
  expect(screen.queryByRole('button', { name: /승인하고 저장|소속 설정|반려|삭제/ })).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '소속 해제' })).toBeInTheDocument();
  expect(screen.getByLabelText('관리 소속')).toHaveTextContent('Own server');
  expect(screen.getByLabelText('관리 소속')).not.toHaveTextContent('Own tenant');
  expect(screen.getByLabelText('추가할 회원 권한')).not.toHaveTextContent('소속 관리자');
});

test('delegated access editor saves disabled access, execution limit and expiry only to its own scope', async () => {
  const manager = { ...admin, id: 'manager', username: 'manager', role: 'viewer', memberships: [scope] };
  const member = { ...pending, status: 'active', memberships: [{ ...scope, role: 'operator' }] };
  mockData(manager, [manager, member]);
  (apiPost as jest.Mock).mockClear();
  render(<UserManagementPanel />);
  fireEvent.click(screen.getByRole('button', { name: /^접근 설정$/ }));
  fireEvent.click(screen.getByLabelText('소속 접근 허용'));
  fireEvent.click(screen.getByLabelText('명령 실행 허용'));
  fireEvent.change(screen.getByLabelText('접근 만료일 (현지 시간)'), { target: { value: '2030-01-01T10:30' } });
  fireEvent.click(screen.getByRole('button', { name: '접근 설정 저장' }));
  await waitFor(() => expect(apiPost).toHaveBeenCalledWith('/api/users/memberships', {
    username: 'new-user', remove: false, membership: {
      kind: 'server', scope_id: 's1', role: 'operator', enabled: false, allow_exec: false,
      expires_at: new Date('2030-01-01T10:30').toISOString(),
    },
  }));
});

import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import RegistrationForm from './RegistrationForm';
import UserManagementPanel from './UserManagementPanel';
import { useApi, apiPost } from '../shared/api/useApi';
import { useMecList } from '../hooks/mec/useMecApi';

jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiRequest: jest.fn() }));
jest.mock('../hooks/mec/useMecApi', () => ({ useMecList: jest.fn() }));
const scope = { kind: 'server', scope_id: 's1', role: 'admin' };
const admin = { id: 'root', username: 'admin', role: 'admin', status: 'active', memberships: [], requested_memberships: [] };
const pending = { id: 'new', username: 'new-user', role: 'viewer', status: 'pending', memberships: [], requested_memberships: [] };
const refetch = jest.fn();
function mockData(me: object, users: object[]) {
  (useApi as jest.Mock).mockImplementation((path: string) => ({
    data: path === '/api/users/me' ? me : path === '/api/users' ? users : [{ id: 's1', name: 'Own server' }],
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
  mockData(admin, [pending]);
  render(<UserManagementPanel />);
  fireEvent.click(screen.getByRole('button', { name: '소속 설정 / 승인' }));
  fireEvent.change(screen.getByLabelText('소속 추가'), { target: { value: 'tenant:t1' } });
  fireEvent.change(screen.getByLabelText('소속 권한'), { target: { value: 'admin' } });
  fireEvent.click(screen.getByRole('button', { name: '승인하고 저장' }));
  await waitFor(() => expect(apiPost).toHaveBeenCalledWith('/api/users/new/review', {
    status: 'active', memberships: [{ kind: 'tenant', scope_id: 't1', role: 'admin' }],
  }));
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

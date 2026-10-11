import React from 'react';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import ManagedMembershipPolicies from './ManagedMembershipPolicies';
import { useApi } from '../../hooks/useApi';
import { useMecList, mecPost } from '../../hooks/mec/useMecApi';
import { I18nProvider } from '../../i18n';
jest.mock('../../hooks/useApi', () => ({ useApi: jest.fn(), fetchWithRefresh: jest.fn() }));
jest.mock('../../hooks/mec/useMecApi', () => ({ useMecList: jest.fn(), mecPost: jest.fn() }));
jest.mock('./TenantAccessDelegation', () => () => null);
jest.mock('./PendingManagedGrants', () => () => null);
jest.mock('./ScopedManagedMemberships', () => () => null);
const refetch = jest.fn();
beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko'); refetch.mockReset(); (mecPost as jest.Mock).mockReset();
  (useApi as jest.Mock).mockReturnValue({ data: [{ id: 'local', username: 'alice', status: 'active', memberships: [{ kind: 'tenant', scope_id: 'tenant', role: 'viewer' }] }, { id: 'foreign', username: 'foreign', status: 'active', memberships: [] }], error: null });
  (useMecList as jest.Mock).mockReturnValue({ data: [], error: null, refetch });
});
const panel = () => render(<I18nProvider><ManagedMembershipPolicies tenantId="tenant" /></I18nProvider>);
it('requires explicit automatic revocation approval and sends stable IDs without creating external grants', async () => {
  (mecPost as jest.Mock).mockResolvedValue({ data: { operation_id: 'membership-one', local_user_id: 'local', automatic_revocation: true } });
  panel();
  expect(screen.queryByRole('option', { name: /foreign/ })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText('승인된 소속 회원'), { target: { value: 'local' } });
  fireEvent.change(screen.getByLabelText('위의 확정된 연결 생성 요청 ID'), { target: { value: 'membership-one' } });
  fireEvent.change(screen.getByLabelText('정책 연결 사유'), { target: { value: '승인된 회원 연결 확인' } });
  const button = screen.getByRole('button', { name: '정책 연결 및 자동 회수 설정' });
  expect(button).toBeDisabled();
  fireEvent.click(within(button.closest('form')!).getByRole('checkbox'));
  fireEvent.click(button);
  await waitFor(() => expect(refetch).toHaveBeenCalledTimes(1));
  expect(mecPost).toHaveBeenCalledWith('/api/mec/v1/tenants/tenant/membership-operations/membership-one/associate', { local_user_id: 'local', confirm_operation_id: 'membership-one', automatic_revocation: true, reason: '승인된 회원 연결 확인' });
});
it('states direct binding absence instead of implying all external access is revoked', () => {
  (useMecList as jest.Mock).mockReturnValue({ data: [{ operation_id: 'one', local_user_id: 'local', source_role: 'viewer', binding_id: 'binding-one', ownership_state: 'revoked' }], error: null, refetch });
  panel(); expect(screen.getByText(/직접 연결 부재 확인됨/)).toBeInTheDocument();
  expect(screen.getByText(/전체 접근 차단을 보증하지 않습니다/)).toBeInTheDocument();
});

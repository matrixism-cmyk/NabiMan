import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import NewManagedGrant from './NewManagedGrant';
import { fetchWithRefresh } from '../../hooks/useApi';
import { mecPost } from '../../hooks/mec/useMecApi';
import { I18nProvider } from '../../i18n';
jest.mock('../../hooks/useApi', () => ({ fetchWithRefresh: jest.fn() }));
jest.mock('../../hooks/mec/useMecApi', () => ({ mecPost: jest.fn() }));
const candidates = [{ id: 'local', username: 'alice', memberships: [{ kind: 'tenant', scope_id: 'tenant', role: 'viewer' }] }];
const onCreated = jest.fn();
beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); (mecPost as jest.Mock).mockReset(); onCreated.mockReset(); (fetchWithRefresh as jest.Mock).mockResolvedValue({ ok: true, json: async () => ({ data: { realm: 'mock://rancher', link: { remote_user_id: 'u-one', remote_username: 'remote-alice' } } }) }); });
async function setup() {
  render(<I18nProvider><NewManagedGrant tenantId="tenant" candidates={candidates} onCreated={onCreated} /></I18nProvider>);
  fireEvent.change(screen.getByLabelText('권한을 부여할 승인 회원'), { target: { value: 'local' } });
  fireEvent.change(screen.getByLabelText('권한 부여 사유'), { target: { value: '승인된 외부 계정 권한 검토' } });
  await screen.findByText('u-one');
}
it('requires both revocation approval and role scope review and preserves the membership ceiling', async () => {
  (mecPost as jest.Mock).mockResolvedValue({ data: { operation_id: 'op', binding_id: 'binding', local_user_id: 'local', automatic_revocation: true, state: 'completed', source_access_currently_allowed: true } });
  await setup();
  expect(screen.queryByRole('option', { name: 'project-owner' })).not.toBeInTheDocument();
  expect(screen.queryByRole('option', { name: 'project-member' })).not.toBeInTheDocument();
  const button = screen.getByRole('button', { name: '관리되는 외부 권한 부여' });
  const checks = screen.getAllByRole('checkbox');
  fireEvent.click(checks[0]); expect(button).toBeDisabled();
  fireEvent.click(checks[1]); fireEvent.click(button);
  await waitFor(() => expect(onCreated).toHaveBeenCalledTimes(1));
  expect(mecPost).toHaveBeenCalledWith('/api/mec/v1/tenants/tenant/managed-memberships', { expected_realm: 'mock://rancher', expected_remote_user_id: 'u-one', local_user_id: 'local', role_template: 'read-only', reason: '승인된 외부 계정 권한 검토', automatic_revocation: true, acknowledge_project_role_scope: true });
});
it('shows pending revocation when source access changed after the remote creation', async () => {
  (mecPost as jest.Mock).mockResolvedValue({ data: { operation_id: 'op', binding_id: 'binding', local_user_id: 'local', automatic_revocation: true, state: 'completed', source_access_currently_allowed: false } });
  await setup(); screen.getAllByRole('checkbox').forEach(checkbox => fireEvent.click(checkbox));
  fireEvent.click(screen.getByRole('button', { name: '관리되는 외부 권한 부여' }));
  await screen.findByText(/현재 소속 접근은 차단돼 있습니다/);
  expect(screen.queryByText('외부 연결과 자동 회수 정책을 함께 저장했습니다.')).not.toBeInTheDocument();
});
it('does not confirm a malformed response or replay creation', async () => {
  (mecPost as jest.Mock).mockResolvedValue({ data: { state: 'completed' } });
  await setup(); screen.getAllByRole('checkbox').forEach(checkbox => fireEvent.click(checkbox));
  fireEvent.click(screen.getByRole('button', { name: '관리되는 외부 권한 부여' }));
  await screen.findByText(/권한 부여 결과를 확인하지 못했습니다/);
  expect(onCreated).not.toHaveBeenCalled(); expect(mecPost).toHaveBeenCalledTimes(1);
});

it('ignores the previous identity response after another member is selected', async () => {
  let release!: (response: unknown) => void;
  (fetchWithRefresh as jest.Mock).mockReset()
    .mockImplementationOnce(() => new Promise(resolve => { release = resolve; }))
    .mockResolvedValueOnce({ ok: true, json: async () => ({ data: { realm: 'mock://rancher', link: { remote_user_id: 'u-other', remote_username: 'remote-other' } } }) });
  render(<I18nProvider><NewManagedGrant tenantId="tenant" candidates={[...candidates, { ...candidates[0], id: 'other', username: 'bob' }]} onCreated={onCreated} /></I18nProvider>);
  fireEvent.change(screen.getByLabelText('권한을 부여할 승인 회원'), { target: { value: 'local' } });
  fireEvent.change(screen.getByLabelText('권한을 부여할 승인 회원'), { target: { value: 'other' } });
  await screen.findByText('u-other');
  release({ ok: true, json: async () => ({ data: { realm: 'mock://rancher', link: { remote_user_id: 'u-one', remote_username: 'remote-alice' } } }) });
  await waitFor(() => expect(fetchWithRefresh).toHaveBeenCalledTimes(2));
  expect(screen.queryByText('u-one')).not.toBeInTheDocument();
  expect(screen.getByText('u-other')).toBeInTheDocument();
});

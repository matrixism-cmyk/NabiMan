import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import MembershipOperations from './MembershipOperations';
import { mecPost, useMecList } from '../../hooks/mec/useMecApi';
import { I18nProvider } from '../../i18n';
jest.mock('../../hooks/mec/useMecApi', () => ({ useMecList: jest.fn(), mecPost: jest.fn() }));
const listMock = useMecList as jest.Mock;
const operation = { id: 'membership-one', operation: 'delete', state: 'unknown_result', realm: 'https://rancher.example', remote_user_id: 'u-one', role_template: 'read-only', binding_id: 'binding-one', actor: 'root', updated_at: '2026-10-05T00:00:00Z', error: 'Provider still reports the binding after deletion' };
beforeEach(() => { listMock.mockReset(); (mecPost as jest.Mock).mockReset(); localStorage.setItem('nabiman_lang', 'ko'); });
const panel = (revision = 0) => <I18nProvider><MembershipOperations tenantId="tenant/one" revision={revision} /></I18nProvider>;
it('preserves an uncertain deletion as unknown and identifies the exact binding without offering replay', () => {
  listMock.mockReturnValue({ data: [operation], error: null, loading: false, refetch: jest.fn() });
  render(panel());
  expect(screen.getByText(/결과 불명/, { selector: 'summary' })).toBeInTheDocument();
  expect(screen.getByText('binding-one', { selector: 'dd' })).toBeInTheDocument();
  expect(screen.getByText('membership-one')).toBeInTheDocument();
  expect(screen.getByRole('status')).toHaveTextContent('자동 재시도는 하지 않습니다');
  expect(screen.getByRole('button', { name: '재확인 후 기록 확정' })).toBeDisabled();
  expect(listMock).toHaveBeenCalledWith('/api/mec/v1/tenants/tenant%2Fone/membership-operations', 30000);
});
it('hides old completion evidence if refreshing the operation history fails', () => {
  listMock.mockReturnValue({ data: [{ ...operation, state: 'completed' }], error: null, loading: false, refetch: jest.fn() });
  const view = render(panel());
  expect(screen.getByText(/결과 확인됨/)).toBeInTheDocument();
  listMock.mockReturnValue({ data: [{ ...operation, state: 'completed' }], error: 'History unavailable', loading: false, refetch: jest.fn() });
  view.rerender(panel());
  expect(screen.getByText('History unavailable')).toBeInTheDocument();
  expect(screen.queryByText(/결과 확인됨/)).not.toBeInTheDocument();
});
it('opens a fresh history after a mutation attempt even when that attempt failed', () => {
  listMock.mockReturnValue({ data: [operation], error: null, loading: false, refetch: jest.fn() });
  const view = render(panel());
  const original = screen.getByText('membership-one');
  view.rerender(panel(1));
  expect(screen.getByText('membership-one')).not.toBe(original);
});

it('requires the exact operation ID and a reason before a confirmation request with the observed version', async () => {
  const refetch = jest.fn().mockResolvedValue(undefined);
  listMock.mockReturnValue({ data: [operation], error: null, loading: false, refetch });
  (mecPost as jest.Mock).mockResolvedValue({ data: { state: 'completed', operation_id: operation.id, provider_write_replayed: false }, error: null });
  render(panel());
  const button = screen.getByRole('button', { name: '재확인 후 기록 확정' });
  fireEvent.change(screen.getByLabelText('재확인 사유'), { target: { value: '외부 연결 조회 확인' } });
  fireEvent.change(screen.getByLabelText('위의 변경 요청 ID를 입력해 확인'), { target: { value: 'wrong' } });
  expect(button).toBeDisabled();
  fireEvent.change(screen.getByLabelText('위의 변경 요청 ID를 입력해 확인'), { target: { value: operation.id } });
  fireEvent.click(button);
  await waitFor(() => expect(refetch).toHaveBeenCalledTimes(1));
  expect(mecPost).toHaveBeenCalledWith('/api/mec/v1/tenants/tenant%2Fone/membership-operations/membership-one/reconcile', { confirm_operation_id: operation.id, expected_binding_id: operation.binding_id, expected_updated_at: operation.updated_at, reason: '외부 연결 조회 확인' });
  expect(button).toBeDisabled();
});
it('does not offer reconciliation when no binding ID was durably observed', () => {
  listMock.mockReturnValue({ data: [{ ...operation, binding_id: null }], error: null, loading: false, refetch: jest.fn() });
  render(panel());
  expect(screen.queryByRole('button', { name: '재확인 후 기록 확정' })).not.toBeInTheDocument();
  expect(screen.getByText(/계정명이나 역할만으로/)).toBeInTheDocument();
});
it('does not report confirmation or replay a write when confirmation fails', async () => {
  listMock.mockReturnValue({ data: [operation], error: null, loading: false, refetch: jest.fn() });
  (mecPost as jest.Mock).mockRejectedValue(new Error('unavailable'));
  render(panel());
  fireEvent.change(screen.getByLabelText('재확인 사유'), { target: { value: '외부 연결 조회 확인' } });
  fireEvent.change(screen.getByLabelText('위의 변경 요청 ID를 입력해 확인'), { target: { value: operation.id } });
  fireEvent.click(screen.getByRole('button', { name: '재확인 후 기록 확정' }));
  await screen.findByText(/기록을 확정하지 못했습니다/);
  expect(screen.queryByText(/이 요청의 직접 연결 결과를 확인했습니다/)).not.toBeInTheDocument();
  expect(mecPost).toHaveBeenCalledTimes(1);
});

import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../i18n';
import { apiRequest } from '../hooks/useApi';
import { useServiceControl } from '../hooks/useServiceControl';
import ServiceControlResult from './ServiceControlResult';
import ServicesPanel from './ServicesPanel';
let mockMe = { id: 'account-one' };
jest.mock('../hooks/useApi', () => ({ apiRequest: jest.fn(), useApi: (endpoint: string) => endpoint === '/api/users/me' ? ({ data: mockMe }) : ({ data: [], loading: false, error: 'Inventory unavailable', refetch: jest.fn() }) }));
const id = '123e4567-e89b-42d3-a456-426614174000';
const refetch = jest.fn();
function Probe() {
  const control = useServiceControl(refetch);
  return <><div role="status">{control.message}</div>
    <button onClick={() => { void control.act('fixture', 'restart'); void control.act('fixture', 'restart'); }}>Change once</button>
    <ServiceControlResult operationId={control.operationId} setOperationId={control.setOperationId}
      result={control.result} busy={control.busy} inspect={control.inspect} /></>;
}
beforeEach(() => {
  jest.clearAllMocks(); sessionStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); mockMe = { id: 'account-one' };
  Object.defineProperty(crypto, 'randomUUID', { configurable: true, value: () => id });
});
it('keeps an id before a lost response, sends only once, and reloads into read-only inspection', async () => {
  (apiRequest as jest.Mock).mockRejectedValueOnce(new Error('EOF'));
  const view = render(<I18nProvider><Probe /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: 'Change once' }));
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('응답을 확인하지 못했습니다'));
  expect(apiRequest).toHaveBeenCalledTimes(1);
  expect(apiRequest).toHaveBeenCalledWith('/api/services/action', expect.objectContaining({ method: 'POST', headers: { 'Idempotency-Key': id } }));
  expect(sessionStorage.getItem('nabiman_service_operation:account-one')).toBe(id);
  expect(refetch).toHaveBeenCalledTimes(1);
  view.unmount();
  (apiRequest as jest.Mock).mockResolvedValueOnce({ success: true, data: { operation_id: id, receipt: {
    operation_id: id, unit: 'fixture.service', action: 'restart', state: 'unknown_result', exit_code: null,
  } } });
  render(<I18nProvider><Probe /></I18nProvider>);
  await waitFor(() => expect(screen.getByRole('textbox')).toHaveValue(id));
  fireEvent.click(screen.getByRole('button', { name: '저장된 결과 확인' }));
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('실행 결과 불명'));
  expect(apiRequest).toHaveBeenLastCalledWith('/api/services/operations/'+id);
  expect(apiRequest).toHaveBeenCalledTimes(2);
});
it('separates remembered IDs by immutable account ID', async () => {
  sessionStorage.setItem('nabiman_service_operation:account-one', id); mockMe = { id: 'account-two' };
  render(<I18nProvider><Probe /></I18nProvider>);
  await waitFor(() => expect(screen.getByRole('textbox')).toHaveValue(''));
  expect(screen.getByRole('button', { name: '저장된 결과 확인' })).toBeDisabled();
  expect(apiRequest).not.toHaveBeenCalled();
});
it('discards an in-flight receipt when the current account changes', async () => {
  let finish!: (value: unknown) => void;
  (apiRequest as jest.Mock).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  const view = render(<I18nProvider><Probe /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: 'Change once' }));
  expect(apiRequest).toHaveBeenCalledTimes(1);
  mockMe = { id: 'account-two' };
  view.rerender(<I18nProvider><Probe /></I18nProvider>);
  await act(async () => { finish({ success: true, data: { operation_id: id, receipt: {
    operation_id: id, unit: 'old-account.service', action: 'restart', state: 'command_failed', exit_code: 1,
  } } }); });
  expect(screen.getByRole('textbox')).toHaveValue('');
  expect(screen.getByRole('status')).toBeEmptyDOMElement();
  expect(screen.queryByText('old-account.service')).not.toBeInTheDocument();
  expect(sessionStorage.getItem('nabiman_service_operation:account-two')).toBeNull();
  expect(refetch).not.toHaveBeenCalled();
});
it('shows unavailable inspection without resubmitting a command', async () => {
  sessionStorage.setItem('nabiman_service_operation:account-one', id);
  (apiRequest as jest.Mock).mockRejectedValueOnce(new Error('Unavailable'));
  render(<I18nProvider><Probe /></I18nProvider>);
  await waitFor(() => expect(screen.getByRole('textbox')).toHaveValue(id));
  fireEvent.click(screen.getByRole('button', { name: '저장된 결과 확인' }));
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('저장된 결과를 확인할 수 없습니다'));
  expect(apiRequest).toHaveBeenCalledTimes(1);
  expect(apiRequest).toHaveBeenCalledWith('/api/services/operations/'+id);
  expect(screen.getByRole('button', { name: '저장된 결과 확인' })).toBeEnabled();
});

it('keeps read-only recovery available when inventory fails without reporting an empty inventory', async () => {
  sessionStorage.setItem('nabiman_service_operation:account-one', id);
  (apiRequest as jest.Mock).mockResolvedValueOnce({ success: true, data: { operation_id: id, receipt: {
    operation_id: id, unit: 'fixture.service', action: 'restart', state: 'command_failed', exit_code: 1,
  } } });
  render(<I18nProvider><ServicesPanel /></I18nProvider>);
  expect(screen.getByRole('alert')).toHaveTextContent('Inventory unavailable');
  expect(screen.queryByText('0 서비스')).not.toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole('textbox', { name: '작업 기록 ID' })).toHaveValue(id));
  fireEvent.click(screen.getByRole('button', { name: '저장된 결과 확인' }));
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('제어 명령 실패'));
  expect(apiRequest).toHaveBeenCalledTimes(1);
  expect(apiRequest).toHaveBeenCalledWith('/api/services/operations/'+id);
});

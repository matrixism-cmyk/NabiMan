import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { useApi, apiPost } from '../shared/api/useApi';
import LicensePanel from './LicensePanel';
jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn() }));
const refetch = jest.fn();
const status = { active: true, tier: 'pro', holder: 'Test holder', expires_at: '2026-10-31', days_remaining: 26, is_trial: false, trial_days_remaining: null, features: ['basic_monitoring'], max_servers: 3, validation_state: 'licensed' };
beforeEach(() => {
  jest.clearAllMocks();
  localStorage.setItem('nabiman_lang', 'ko');
  (useApi as jest.Mock).mockImplementation((url: string) => ({ data: url.endsWith('/features') ? status.features : status, loading: false, error: null, refetch }));
});
function activate() {
  const input = screen.getByRole('textbox');
  fireEvent.change(input, { target: { value: 'NBL1.test-fixture' } });
  fireEvent.click(screen.getByRole('button', { name: '활성화' }));
  return input;
}
it('renders the status object activation response as a success message and clears the key', async () => {
  (apiPost as jest.Mock).mockResolvedValue({ success: true, data: status, message: '' });
  render(<I18nProvider><LicensePanel /></I18nProvider>);
  const input = activate();
  await screen.findByText('라이선스가 활성화되었습니다');
  expect(input).toHaveValue('');
  expect(refetch).toHaveBeenCalledTimes(1);
});
it('keeps invalid keys editable and does not claim activation or refresh after rejection', async () => {
  (apiPost as jest.Mock).mockResolvedValue({ success: false, data: null, message: 'Invalid license signature' });
  render(<I18nProvider><LicensePanel /></I18nProvider>);
  const input = activate();
  await screen.findByText('Invalid license signature');
  expect(input).toHaveValue('NBL1.test-fixture');
  expect(refetch).not.toHaveBeenCalled();
  expect(screen.getByRole('button', { name: '활성화' })).toBeEnabled();
});
it('re-enables activation after a network error', async () => {
  (apiPost as jest.Mock).mockRejectedValue(new Error('Network unavailable'));
  render(<I18nProvider><LicensePanel /></I18nProvider>);
  activate();
  await screen.findByText('Network unavailable');
  await waitFor(() => expect(screen.getByRole('button', { name: '활성화' })).toBeEnabled());
});
it('explains expired verification state without presenting a perpetual license', () => {
  (useApi as jest.Mock).mockImplementation((url: string) => ({ data: url.endsWith('/features') ? ['basic_monitoring'] : { ...status, active: false, tier: 'free', holder: '', expires_at: null, days_remaining: null, validation_state: 'license_expired' }, loading: false, error: null, refetch }));
  render(<I18nProvider><LicensePanel /></I18nProvider>);
  expect(screen.getByRole('alert')).toHaveTextContent('라이선스가 만료되었습니다.');
  expect(screen.queryByText('무기한')).not.toBeInTheDocument();
});
it('shows a plan with its usage, features and grace notice in the chosen language', () => {
  const planned = {
    ...status, validation_state: 'grace', grace_ends_at: '2026-11-14', grace_days_remaining: 3,
    limits: { plan: 'team', deployment: 'installed', max_users: 5, max_servers: 10, features: ['recording'] },
    usage: { users: 5, servers: 2 }, notices: ['grace', 'users_at_limit'],
  };
  (useApi as jest.Mock).mockImplementation((url: string) => ({ data: url.endsWith('/features') ? status.features : planned, loading: false, error: null, refetch }));
  render(<I18nProvider><LicensePanel /></I18nProvider>);
  expect(screen.getByText('라이선스가 만료되어 유예 기간입니다.')).toBeInTheDocument();
  expect(screen.getByText(/유예 기간이 2026-11-14까지/)).toBeInTheDocument();
  expect(screen.getByText(/라이선스 한도\(5명\)에 도달/)).toBeInTheDocument();
  expect(screen.getByText('5 / 5')).toBeInTheDocument();
  expect(screen.getByText('2 / 10')).toBeInTheDocument();
  expect(screen.getByText('설치형')).toBeInTheDocument();
  expect(screen.getByText('세션 녹화').previousSibling).toHaveTextContent('허용');
  expect(screen.getByText('MEC 관리').previousSibling).toHaveTextContent('미허용');
});
it('shows no plan section for a licence without a plan', () => {
  render(<I18nProvider><LicensePanel /></I18nProvider>);
  expect(screen.queryByText('요금제')).not.toBeInTheDocument();
  expect(screen.queryByRole('list')).not.toBeInTheDocument();
});

import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { useApi } from '../shared/api/useApi';
import LicenseBanner from './LicenseBanner';
jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn() }));
const base = { active: true, tier: 'pro', holder: 'h', expires_at: '2026-11-20', days_remaining: 9, is_trial: false, trial_days_remaining: null, features: [], max_servers: 10, validation_state: 'licensed' };
const answer = (data: unknown) => (useApi as jest.Mock).mockReturnValue({ data, loading: false, error: null, refetch: jest.fn() });
beforeEach(() => { jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en'); });

it('says nothing for a licence without a plan, and asks nothing when disabled', () => {
  answer(base);
  const { container, rerender } = render(<I18nProvider><LicenseBanner enabled onOpen={jest.fn()} /></I18nProvider>);
  expect(container).toBeEmptyDOMElement();
  rerender(<I18nProvider><LicenseBanner enabled={false} onOpen={jest.fn()} /></I18nProvider>);
  expect(useApi).toHaveBeenLastCalledWith('', 3_600_000);
});
it('warns about limits and expiry, opens the licence screen and can be hidden', () => {
  answer({ ...base, limits: { plan: 'team', max_servers: 10, features: [] }, usage: { users: 3, servers: 12 }, notices: ['expiring_soon', 'servers_over_limit'] });
  const onOpen = jest.fn();
  render(<I18nProvider><LicenseBanner enabled onOpen={onOpen} /></I18nProvider>);
  const banner = screen.getByRole('status', { name: 'License notice' });
  expect(banner).toHaveTextContent('The license expires in 9 days (2026-11-20).');
  expect(banner).toHaveTextContent('Saved servers (12) exceed the license limit (10).');
  expect(banner).toHaveClass('serious');
  fireEvent.click(screen.getByRole('button', { name: 'View license' }));
  expect(onOpen).toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: 'Close' }));
  expect(screen.queryByRole('status')).not.toBeInTheDocument();
});

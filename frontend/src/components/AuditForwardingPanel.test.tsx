import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiPost, useApi } from '../shared/api/useApi';
import AuditForwardingPanel from './AuditForwardingPanel';
jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn() }));
beforeEach(() => { jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en'); (useApi as jest.Mock).mockReturnValue({ data: { destinations: [], counters: { acknowledged: 0, cancelled: 0 }, legacy_requires_review: false }, refetch: jest.fn() }); });
it('clears the authorization input even when saving fails and never enables implicitly', async () => {
  (apiPost as jest.Mock).mockResolvedValue({ success: false, message: 'Storage unavailable' });
  render(<I18nProvider><AuditForwardingPanel /></I18nProvider>);
  fireEvent.change(screen.getByLabelText('Receiver name'), { target: { value: 'Audit receiver' } });
  fireEvent.change(screen.getByLabelText('HTTPS receiver URL'), { target: { value: 'https://collector.example/events' } });
  fireEvent.change(screen.getByLabelText('Authorization header · optional'), { target: { value: 'Bearer fixture-secret' } });
  fireEvent.click(screen.getByRole('button', { name: 'Save disabled receiver' }));
  expect(screen.getByLabelText('Authorization header · optional')).toHaveValue('');
  expect(await screen.findByRole('alert')).toHaveTextContent('Storage unavailable');
  expect(apiPost).toHaveBeenCalledTimes(1);
  expect(apiPost).toHaveBeenCalledWith('/api/audit/forwarding', { name: 'Audit receiver', url: 'https://collector.example/events', auth_header: 'Bearer fixture-secret' });
});
it('shows legacy review and capture gaps without claiming delivery success', () => {
  (useApi as jest.Mock).mockReturnValue({ data: { destinations: [], counters: { acknowledged: 0, cancelled: 0, capture_failed: 3 }, legacy_requires_review: true, delivery_paused_by_operator: true } });
  render(<I18nProvider><AuditForwardingPanel /></I18nProvider>);
  expect(screen.getByText(/Legacy configuration is preserved/)).toBeInTheDocument();
  expect(screen.getByText(/Some events were not captured/)).toBeInTheDocument();
  expect(screen.getByText(/External delivery is paused/)).toBeInTheDocument();
});

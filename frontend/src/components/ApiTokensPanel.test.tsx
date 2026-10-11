import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiPost, useApi } from '../hooks/useApi';
import ApiTokensPanel from './ApiTokensPanel';
jest.mock('../hooks/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiRequest: jest.fn() }));
beforeEach(() => { jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en'); (useApi as jest.Mock).mockReturnValue({ data: [], refetch: jest.fn() }); });
it('uses bounded expiry and explicit scopes without sending a global role', async () => {
  (apiPost as jest.Mock).mockResolvedValue({ success: true, data: { id: 'test', key: 'nbm_fixture', expires_at: 'tomorrow' } });
  render(<I18nProvider><ApiTokensPanel /></I18nProvider>);
  fireEvent.change(screen.getByLabelText('Key Name'), { target: { value: 'Monitor' } });
  fireEvent.click(screen.getByRole('button', { name: 'Generate Key' }));
  expect(await screen.findByTestId('issued-token')).toHaveTextContent('nbm_fixture');
  expect(apiPost).toHaveBeenCalledWith('/api/apikeys', { name: 'Monitor', scopes: ['status:read'], expires_in_days: 7 });
});
it('does not offer creation when token storage cannot be read', () => {
  (useApi as jest.Mock).mockReturnValue({ data: null, error: 'Storage unavailable' });
  render(<I18nProvider><ApiTokensPanel /></I18nProvider>);
  expect(screen.getByRole('alert')).toHaveTextContent('Storage unavailable');
  expect(screen.getByRole('button', { name: 'Generate Key' })).toBeDisabled();
});

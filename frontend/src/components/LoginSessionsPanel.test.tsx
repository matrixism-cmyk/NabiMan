import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { useApi, apiPost } from '../hooks/useApi';
import LoginSessionsPanel from './LoginSessionsPanel';
jest.mock('../hooks/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn() }));
it('does not report successful revocation when the server rejects it', async () => {
  localStorage.setItem('nabiman_lang', 'en');
  const confirm = jest.spyOn(window, 'confirm').mockReturnValue(true);
  const refetch = jest.fn();
  (useApi as jest.Mock).mockReturnValue({ loading: false, error: null, refetch,
    data: [{ session_id: 'current', username: 'member', is_current: true },
      { session_id: 'other', username: 'member', is_current: false }] });
  (apiPost as jest.Mock).mockResolvedValue({ success: false, message: 'Session not found' });
  try {
    render(<I18nProvider><LoginSessionsPanel /></I18nProvider>);
    expect(screen.getAllByRole('button', { name: 'Sign out' })).toHaveLength(1);
    fireEvent.click(screen.getByRole('button', { name: 'Sign out' }));
    await screen.findByText('Session not found');
    expect(screen.queryByText('Login sessions revoked.')).not.toBeInTheDocument();
    expect(refetch).not.toHaveBeenCalled();
    expect(apiPost).toHaveBeenCalledWith('/api/auth/sessions/revoke', { session_id: 'other' });
    expect(screen.getByText('Current login')).toBeInTheDocument();
  } finally { confirm.mockRestore(); }
});

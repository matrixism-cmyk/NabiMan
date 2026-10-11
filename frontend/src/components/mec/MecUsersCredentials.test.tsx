import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { fetchWithRefresh } from '../../hooks/useApi';
import MecUsersPanel from './MecUsersPanel';
jest.mock('../../hooks/useApi', () => ({ fetchWithRefresh: jest.fn() }));
jest.mock('../../hooks/mec/useMecApi', () => ({ useMecList: () => ({ data: [{ id: 'u-test', username: 'test-user', enabled: true }], refetch: jest.fn() }) }));
it('does not disclose a new credential or claim success after a rejected reset', async () => {
  localStorage.setItem('nabiman_lang', 'ko');
  (fetchWithRefresh as jest.Mock).mockResolvedValue({ ok: false, status: 403, json: async () => ({ error: { message: 'Reset denied' } }) });
  const prompt = jest.spyOn(window, 'prompt').mockReturnValue('isolated-test-new-password');
  try {
    render(<I18nProvider><MecUsersPanel /></I18nProvider>);
    fireEvent.click(screen.getByRole('button', { name: '비번 리셋' }));
    await screen.findByText('Error: Reset denied');
    expect(screen.queryByLabelText('초기 비밀번호')).not.toBeInTheDocument();
  } finally { prompt.mockRestore(); }
});

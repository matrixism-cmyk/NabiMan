import React, { useState } from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../i18n';
import { apiRequest } from '../../shared/api/useApi';
import { DEFAULT_TERMINAL_SETTINGS, TerminalSettingsProvider, useTerminalSettings } from './settings';
jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
function Probe() {
  const { settings, save, loadError } = useTerminalSettings();
  const [error, setError] = useState('');
  return <><output>{settings.font_size}</output><div role="alert">{error || loadError}</div><button onClick={async () => setError(await save({ ...settings, font_size: 25 }))}>Save fixture</button></>;
}
beforeEach(() => { jest.clearAllMocks(); localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); });
it('shows a load failure instead of claiming cached preferences came from the server', async () => {
  localStorage.setItem('nabiman_terminal_settings', JSON.stringify({ ...DEFAULT_TERMINAL_SETTINGS, font_size: 19 }));
  (apiRequest as jest.Mock).mockRejectedValueOnce(new Error('Unavailable'));
  render(<I18nProvider><TerminalSettingsProvider><Probe /></TerminalSettingsProvider></I18nProvider>);
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('환경설정을 불러오지 못했습니다'));
  expect(screen.getByRole('status')).toHaveTextContent('19');
  expect(apiRequest).toHaveBeenCalledTimes(1);
});
it.each(['response', 'network'])('preserves confirmed settings and cache when saving fails: %s', async (failure) => {
  const confirmed = { ...DEFAULT_TERMINAL_SETTINGS, font_size: 20 };
  (apiRequest as jest.Mock).mockResolvedValueOnce({ success: true, data: confirmed });
  render(<I18nProvider><TerminalSettingsProvider><Probe /></TerminalSettingsProvider></I18nProvider>);
  await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('20'));
  if (failure === 'network') (apiRequest as jest.Mock).mockRejectedValueOnce(new Error('EOF'));
  else (apiRequest as jest.Mock).mockResolvedValueOnce({ success: false, message: 'Unavailable' });
  fireEvent.click(screen.getByRole('button', { name: 'Save fixture' }));
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('저장 결과를 확인하지 못했습니다'));
  expect(screen.getByRole('status')).toHaveTextContent('20');
  expect(JSON.parse(localStorage.getItem('nabiman_terminal_settings') || '{}').font_size).toBe(20);
  expect(apiRequest).toHaveBeenCalledTimes(2);
});

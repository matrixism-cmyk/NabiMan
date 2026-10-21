import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { apiPost, fetchWithRefresh } from '../shared/api/useApi';
import { I18nProvider } from '../i18n';
import FileManagerPanel from './FileManagerPanel';
jest.mock('../shared/api/useApi', () => ({ apiPost: jest.fn(), fetchWithRefresh: jest.fn() }));
beforeEach(() => {
  jest.clearAllMocks();
  localStorage.setItem('nabiman_lang', 'ko');
  sessionStorage.setItem('nabiman_token', 'never-download-in-url');
  (apiPost as jest.Mock).mockResolvedValue({ success: true });
  global.fetch = jest.fn().mockResolvedValue({ json: async () => ({ success: true, data: [{ name: 'report.txt', path: '/report.txt', is_dir: false, size: 10, modified: 'now', permissions: '600' }] }) });
});
it('downloads through the refresh-aware header client without putting the JWT in a URL', async () => {
  const blob = new Blob(['test']);
  (fetchWithRefresh as jest.Mock).mockResolvedValue({ ok: true, blob: async () => blob });
  URL.createObjectURL = jest.fn().mockReturnValue('blob:test-download');
  URL.revokeObjectURL = jest.fn();
  const click = jest.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
  try {
    render(<I18nProvider><FileManagerPanel /></I18nProvider>);
    fireEvent.click(await screen.findByRole('button', { name: /다운로드/ }));
    await waitFor(() => expect(click).toHaveBeenCalled());
    expect(fetchWithRefresh).toHaveBeenCalledWith('/api/files/download?path=%2Freport.txt');
    expect(URL.createObjectURL).toHaveBeenCalledWith(blob);
  } finally { click.mockRestore(); }
});
it('shows download rejection without creating a downloadable credential-bearing address', async () => {
  (fetchWithRefresh as jest.Mock).mockResolvedValue({ ok: false, status: 403 });
  URL.createObjectURL = jest.fn();
  render(<I18nProvider><FileManagerPanel /></I18nProvider>);
  fireEvent.click(await screen.findByRole('button', { name: /다운로드/ }));
  await screen.findByText('HTTP 403');
  expect(URL.createObjectURL).not.toHaveBeenCalled();
});

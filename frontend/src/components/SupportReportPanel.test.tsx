import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiPost, useApi } from '../hooks/useApi';
import SupportReportPanel from './SupportReportPanel';
jest.mock('../hooks/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn() }));
const section = { state: 'ready', data: {} };
const report = { schema_version: 1, report_id: 'previous-report', generated_at: '2026-10-06T00:00:00Z', process: { runs_as_root: false }, accounts: section, login_sessions: section, terminal_registry: section, server_inventory: section, auth_storage: section, recent_jobs: section };
beforeEach(() => { jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en'); (useApi as jest.Mock).mockReturnValue({ data: { role: 'admin' } }); });
it('removes a reviewed previous report when a new collection fails', async () => {
  (apiPost as jest.Mock).mockResolvedValueOnce({ success: true, data: report }).mockRejectedValueOnce(new Error('Collection unavailable'));
  render(<I18nProvider><SupportReportPanel /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: 'Generate diagnostic report' }));
  await screen.findByLabelText('Diagnostic download preview');
  fireEvent.click(screen.getByLabelText('I have reviewed the contents and want to download this report'));
  expect(screen.getByRole('button', { name: 'Download reviewed JSON' })).toBeEnabled();
  fireEvent.click(screen.getByRole('button', { name: 'Generate diagnostic report' }));
  await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('Collection unavailable'));
  expect(screen.queryByLabelText('Diagnostic download preview')).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Download reviewed JSON' })).not.toBeInTheDocument();
});
it('does not expose the collector to ordinary members or collect automatically', () => {
  (useApi as jest.Mock).mockReturnValue({ data: { role: 'viewer' } });
  const view = render(<I18nProvider><SupportReportPanel /></I18nProvider>);
  expect(view.container).toBeEmptyDOMElement(); expect(apiPost).not.toHaveBeenCalled();
});

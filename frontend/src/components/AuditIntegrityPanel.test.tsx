import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiPost, apiRequest, useApi } from '../hooks/useApi';
import AuditIntegrityPanel from './AuditIntegrityPanel';
jest.mock('../hooks/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiRequest: jest.fn() }));
const segment = { name: 'audit.jsonl', valid: false, entries_checked: 2, malformed_lines: 1, hash_mismatches: 1, unhashed_entries: 0, truncated: false, sha256: 'source-digest', error: null };
beforeEach(() => {
  jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en');
  (useApi as jest.Mock).mockReturnValue({ data: { writable: true } });
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: { valid: false, entries_checked: 2, segments: [segment] } });
});
it('reports unavailable persistence and never calls it a healthy empty audit', () => {
  (useApi as jest.Mock).mockReturnValue({ data: { writable: false, error: 'write_failed' } });
  render(<I18nProvider><AuditIntegrityPanel /></I18nProvider>);
  expect(screen.getByRole('alert')).toHaveTextContent('Audit persistence stopped');
  expect(apiRequest).not.toHaveBeenCalled(); expect(apiPost).not.toHaveBeenCalled();
});
it('shows legacy inconsistencies and clears export consent when preparation fails', async () => {
  (apiPost as jest.Mock).mockResolvedValueOnce({ success: true, data: { segment: 'audit.jsonl', verification: segment, entries: [{ path: '/safe' }] } }).mockRejectedValueOnce(new Error('Source unavailable'));
  render(<I18nProvider><AuditIntegrityPanel /></I18nProvider>);
  fireEvent.click(screen.getByRole('button', { name: 'Inspect retained audit files' }));
  expect(await screen.findByText(/Integrity needs review/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: 'Prepare export' }));
  await screen.findByLabelText('Audit export preview · first 20 entries');
  expect(screen.getByRole('button', { name: 'Download reviewed audit JSON' })).toBeDisabled();
  fireEvent.click(screen.getByLabelText('I reviewed the scope and sensitive-data notice'));
  expect(screen.getByRole('button', { name: 'Download reviewed audit JSON' })).toBeEnabled();
  fireEvent.click(screen.getByRole('button', { name: 'Prepare export' }));
  await screen.findByText('Source unavailable');
  expect(screen.queryByRole('button', { name: 'Download reviewed audit JSON' })).not.toBeInTheDocument();
});

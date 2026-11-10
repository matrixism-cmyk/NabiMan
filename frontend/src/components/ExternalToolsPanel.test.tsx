import React from 'react';
import { render, screen, within } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { useApi } from '../shared/api/useApi';
import ExternalToolsPanel from './ExternalToolsPanel';
jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiRequest: jest.fn() }));

it('shows each program, whether it was found, and what it enables', () => {
  localStorage.setItem('nabiman_lang', 'en');
  (useApi as jest.Mock).mockReturnValue({ data: { tools: { checked_at: '2026-11-10T00:00:00Z', tools: [
    { name: 'tmux', found: true, path: '/usr/bin/tmux', version: 'tmux 3.8', features: ['terminal'] },
    { name: 'guacd', found: false, path: '127.0.0.1:4822', version: null, features: ['rdp_console'] },
    { name: 'ffmpeg', found: false, path: null, version: null, features: [] },
  ] } }, loading: false, error: null, refetch: jest.fn() });
  render(<I18nProvider><ExternalToolsPanel /></I18nProvider>);
  const rows = screen.getAllByRole('row');
  expect(within(rows[1]).getByText('tmux 3.8')).toBeInTheDocument();
  expect(within(rows[1]).getByText('Found')).toBeInTheDocument();
  expect(within(rows[1]).getByText('Terminals')).toBeInTheDocument();
  expect(within(rows[2]).getByText('Missing — these are off')).toBeInTheDocument();
  expect(within(rows[2]).getByText('RDP consoles')).toBeInTheDocument();
  expect(within(rows[3]).getByText('Missing (not used by this version)')).toBeInTheDocument();
});

it('says when there is no report yet', () => {
  (useApi as jest.Mock).mockReturnValue({ data: { tools: null }, loading: false, error: null, refetch: jest.fn() });
  render(<I18nProvider><ExternalToolsPanel /></I18nProvider>);
  expect(screen.getByText('Not checked yet.')).toBeInTheDocument();
});

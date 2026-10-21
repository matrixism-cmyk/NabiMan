import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { useApi } from '../shared/api/useApi';
import RuntimeIdentityPanel from './RuntimeIdentityPanel';
jest.mock('./ReleaseInventoryPanel', () => () => null);
jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn() }));
it('hides a previously observed executable when refresh fails and permits retry', () => {
  localStorage.setItem('nabiman_lang', 'ko');
  const refetch = jest.fn();
  const result = { data: { package_version: 'test', executable_sha256: 'previous-digest',
    instance_id: 'previous-instance', started_at: '2026-10-05T00:00:00Z' },
    loading: false, error: null as string | null, refetch };
  (useApi as jest.Mock).mockImplementation(() => result);
  const view = render(<I18nProvider><RuntimeIdentityPanel /></I18nProvider>);
  expect(screen.getByText('previous-digest')).toBeInTheDocument();
  result.error = 'Connection lost';
  view.rerender(<I18nProvider><RuntimeIdentityPanel /></I18nProvider>);
  expect(screen.queryByText('previous-digest')).not.toBeInTheDocument();
  expect(screen.getByRole('alert')).toHaveTextContent('Connection lost');
  fireEvent.click(screen.getByRole('button'));
  expect(refetch).toHaveBeenCalledTimes(1);
});

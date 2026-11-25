import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import App from '../../App';
import { forgetBranding } from '../../shared/branding/branding';

jest.mock('../terminal/TerminalView', () => {
  const react = require('react');
  return { __esModule: true, default: react.forwardRef(function FakeTerminalView() { return react.createElement('div'); }) };
});

let fetchMock: jest.SpyInstance;
function signedIn(rack: boolean | undefined) {
  window.history.replaceState(null, '', '/');
  sessionStorage.clear(); localStorage.clear(); forgetBranding();
  localStorage.setItem('nabiman_lang', 'ko');
  sessionStorage.setItem('nabiman_token', 'test-session');
  fetchMock = jest.spyOn(global, 'fetch').mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input);
    const data = url.endsWith('/api/users/me') ? { username: 'admin', role: 'admin', memberships: [], scope_restricted: false }
      : url.endsWith('/api/branding') ? { product_name: 'NabiMan', feedback_widget: null, noc_contact: null, rack }
      : url.endsWith('/provider') ? { enabled: false } : false;
    return { status: 200, ok: true, json: async () => ({ success: true, data }) } as Response;
  });
  return render(<I18nProvider><App /></I18nProvider>);
}
afterEach(() => { fetchMock.mockRestore(); });

async function openMec() {
  const mec = await screen.findByRole('button', { name: 'MEC' });
  fireEvent.click(mec);
  await screen.findByText('통합관제');
}

test('the menu has "서버실 랙" under MEC when the install has the rack page', async () => {
  signedIn(true);
  await waitFor(() => expect(fetchMock.mock.calls.some(([url]) => String(url).endsWith('/api/branding'))).toBe(true));
  await openMec();
  expect(await screen.findByText('서버실 랙')).toBeInTheDocument();
});

test('the entry is hidden when the feature is off, or an older server says nothing about it', async () => {
  for (const rack of [false, undefined]) {
    const view = signedIn(rack);
    await waitFor(() => expect(fetchMock.mock.calls.some(([url]) => String(url).endsWith('/api/branding'))).toBe(true));
    await openMec();
    expect(screen.queryByText('서버실 랙')).toBeNull();
    view.unmount();
    fetchMock.mockRestore();
  }
});

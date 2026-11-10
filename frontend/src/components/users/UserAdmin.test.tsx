import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import UserAdmin from './UserAdmin';
import { I18nProvider } from '../../i18n';
import { useApi, apiRequest } from '../../shared/api/useApi';
import { useMecList } from '../../hooks/mec/useMecApi';

jest.mock('../../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiRequest: jest.fn() }));
jest.mock('../../hooks/mec/useMecApi', () => ({ useMecList: jest.fn() }));

const HANGUL = /[가-힣]/;
const admin = { id: 'root', username: 'admin', role: 'admin', status: 'active', memberships: [], requested_memberships: [], created_at: '2026-01-01T00:00:00Z', last_login: null };
const remote = {
  id: 'rm', username: 'remote-user', role: 'remote', status: 'active', totp_enabled: false,
  memberships: [{ kind: 'server', scope_id: 's1', role: 'viewer' }], requested_memberships: [],
  access: { expires_at: '2030-01-01T00:00:00Z', require_mfa: true, allowed_networks: ['203.0.113.0/24'], allowed_hours: { start: 9, end: 18, days: [0] } },
  created_at: '2026-01-01T00:00:00Z', last_login: '2026-11-01T00:00:00Z',
};
const pending = { id: 'new', username: 'new-user', role: 'viewer', status: 'pending', memberships: [], requested_memberships: [{ kind: 'tenant', scope_id: 't1', role: 'operator' }], created_at: '2026-01-01T00:00:00Z', last_login: null };
const overview = {
  own_servers: [], assigned_servers: [{ id: 's1', name: 'Own server', address: '10.0.0.1', status: 'online' }],
  terminal_sessions: [{ id: 'x', label: 'shell', title: '', created_unix: 1, last_active_unix: 2, keepalive: true, viewers: 1 }],
  share_links: [{ token: 'k', label: 'link', expires_unix: 253402300799, files: 'write', read_only: true, usable: false, needs_password: true }],
  sign_ins: [{ session_id: 'a', created_at: '2026-11-01T00:00:00Z', last_active: '2026-11-01T00:00:00Z', ip: '' }],
};

/** Every word on the page and in its labels, placeholders and titles. */
function shownText(root: HTMLElement): string {
  const attrs = Array.from(root.querySelectorAll('[aria-label],[placeholder],[title]'))
    .map(el => ['aria-label', 'placeholder', 'title'].map(a => el.getAttribute(a) || '').join(' '));
  return `${root.textContent} ${attrs.join(' ')}`;
}

// Dates follow the machine's locale, not the page's language; pin it so a
// Korean test machine does not count its own 오전/오후 as leftover words.
const toLocaleString = Date.prototype.toLocaleString;
const toLocaleDateString = Date.prototype.toLocaleDateString;
beforeEach(() => {
  jest.spyOn(Date.prototype, 'toLocaleString').mockImplementation(function (this: Date) { return toLocaleString.call(this, 'en-US'); });
  jest.spyOn(Date.prototype, 'toLocaleDateString').mockImplementation(function (this: Date) { return toLocaleDateString.call(this, 'en-US'); });
  localStorage.setItem('nabiman_lang', 'en');
  (useApi as jest.Mock).mockImplementation((path: string) => ({
    data: path === '/api/users' ? [admin, remote, pending] : path.endsWith('/overview') ? overview : [{ id: 's1', name: 'Own server' }],
    loading: false, error: null, refetch: jest.fn(),
  }));
  (useMecList as jest.Mock).mockReturnValue({ data: [{ id: 't1', display_name: 'Own tenant' }] });
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: '203.0.113.5' });
});

test('the member list and every tab of a member read in English with no Korean left', async () => {
  const { container } = render(<I18nProvider><UserAdmin me={admin as never} /></I18nProvider>);
  expect(screen.getByRole('heading', { name: 'Super administrator · member approval and memberships' })).toBeInTheDocument();
  expect(shownText(container)).not.toMatch(HANGUL);
  expect(container.textContent).toContain('2030');

  fireEvent.click(screen.getByText('remote-user', { selector: 'strong' }));
  for (const tab of ['Overview', 'Account', 'Assigned servers', 'Remote permissions', 'Access limits']) {
    fireEvent.click(screen.getByRole('tab', { name: tab }));
    if (tab === 'Access limits') await screen.findByText('203.0.113.5');
    expect(shownText(container)).not.toMatch(HANGUL);
  }

  fireEvent.click(screen.getByText('new-user', { selector: 'strong' }));
  expect(screen.getByRole('button', { name: 'Approve and save' })).toBeInTheDocument();
  expect(shownText(container)).not.toMatch(HANGUL);

  fireEvent.click(screen.getByRole('button', { name: '+ Add member' }));
  expect(shownText(document.body)).not.toMatch(HANGUL);
});

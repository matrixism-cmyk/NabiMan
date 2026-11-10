import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import ScopeSettingsPanel from './ScopeSettingsPanel';
import { I18nProvider } from '../i18n';
import { useApi } from '../shared/api/useApi';
import { useMecList } from '../hooks/mec/useMecApi';

jest.mock('../shared/api/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiRequest: jest.fn() }));
jest.mock('../hooks/mec/useMecApi', () => ({ useMecList: jest.fn() }));

const HANGUL = /[가-힣]/;
const admin = { id: 'root', username: 'admin', role: 'admin', status: 'active', memberships: [], requested_memberships: [] };
const settings = { revision: 'r2', contact_email: 'ops@example.com', member_notice: 'Be nice', member_limit: 5, allow_commands: true, require_mfa: false };
const history = [{
  revision: 'r2', at: '2026-11-01T00:00:00Z', actor: 'admin', actor_id: 'root', reason: 'tighten',
  fields: [{ field: 'require_mfa', before: true, after: false }, { field: 'member_limit', before: null, after: 5 }],
}];
const tenant = {
  id: 't1', display_name: 'Own tenant', namespace: 'own',
  quota: { cpu_requests: '2', cpu_limits: '4', memory_requests: '4Gi', memory_limits: '8Gi', gpu: 0, storage: '50Gi', pods: 20, pvcs: 5, lb_services: 1 },
};

function shownText(root: HTMLElement): string {
  const attrs = Array.from(root.querySelectorAll('[aria-label],[placeholder],[title]'))
    .map(el => ['aria-label', 'placeholder', 'title'].map(a => el.getAttribute(a) || '').join(' '));
  return `${root.textContent} ${attrs.join(' ')}`;
}

// Dates follow the machine's locale, not the page's language.
const toLocaleString = Date.prototype.toLocaleString;
beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'en');
  jest.spyOn(Date.prototype, 'toLocaleString').mockImplementation(function (this: Date) { return toLocaleString.call(this, 'en-US'); });
  (useApi as jest.Mock).mockImplementation((path: string) => ({
    data: path === '/api/users/me' ? admin
      : path === '/api/remote-servers' ? [{ id: 's1', name: 'Own server' }]
      : path.endsWith('/history') ? history
      : path.startsWith('/api/scope-settings/') ? settings
      : [],
    loading: false, error: null, refetch: jest.fn(),
  }));
  (useMecList as jest.Mock).mockReturnValue({ data: [tenant], error: null });
});

test('scope settings, quota and policy history read in English with no Korean left', () => {
  const { container } = render(<I18nProvider><ScopeSettingsPanel /></I18nProvider>);
  expect(screen.getByRole('heading', { name: 'tenant & server settings' })).toBeInTheDocument();
  expect(screen.getByRole('heading', { name: 'tenant resource quota' })).toBeInTheDocument();
  expect(screen.getByText('2-step verification required')).toBeInTheDocument();
  expect(shownText(container)).not.toMatch(HANGUL);

  fireEvent.change(screen.getByLabelText(/Scope to manage/), { target: { value: 'server:s1' } });
  expect(screen.getByText(/Let members run commands on the server/)).toBeInTheDocument();
  expect(shownText(container)).not.toMatch(HANGUL);
});

test('the same panel still reads in Korean', () => {
  localStorage.setItem('nabiman_lang', 'ko');
  render(<I18nProvider><ScopeSettingsPanel /></I18nProvider>);
  expect(screen.getByRole('heading', { name: 'tenant·서버 운영 설정' })).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '운영 설정 저장' })).toBeInTheDocument();
});

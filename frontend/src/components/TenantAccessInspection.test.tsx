import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import TenantAccessInspection from './TenantAccessInspection';
import { fetchWithRefresh } from '../shared/api/useApi';
import { I18nProvider } from '../i18n';
jest.mock('../shared/api/useApi', () => ({ fetchWithRefresh: jest.fn() }));
const fetchMock = fetchWithRefresh as jest.Mock;
const report = {
  local_username: 'alice', inspected_at: '2026-10-05T00:00:00Z', source_access_allowed: false,
  realm: 'https://rancher.example', project_id: 'c:p', reference_project_role: null,
  identity_link: { remote_user_id: 'u-1', remote_username: 'external-alice' },
  remote_account_enabled: true, direct_project_role_matches: false,
  direct_bindings: [{ id: 'binding-1', user_id: 'u-1', role_template: 'project-owner' }],
  blockers: ['local_access_denied'], unverified_access_sources: ['global_roles', 'cluster_roles', 'group_membership', 'other_project_roles', 'custom_role_rules'],
};
beforeEach(() => { fetchMock.mockReset(); localStorage.setItem('nabiman_lang', 'ko'); });
const renderPanel = () => render(<I18nProvider><TenantAccessInspection tenantId="tenant-a" userId="alice" onClose={() => {}} /></I18nProvider>);
it('shows provider access even when local access was revoked without presenting full effective access', async () => {
  fetchMock.mockResolvedValue({ ok: true, json: async () => ({ data: report, error: null }) });
  renderPanel();
  await screen.findByText('project-owner');
  expect(screen.getByText('차단 / 비활성')).toBeInTheDocument();
  expect(screen.getByText('차이 또는 미확인 항목 있음')).toBeInTheDocument();
  expect(screen.getByText('외부 그룹 소속')).toBeInTheDocument();
  expect(screen.getByText(/전체 접근 권한을 보증하지 않습니다/)).toBeInTheDocument();
  expect(fetchMock.mock.calls[0][0]).toBe('/api/mec/v1/tenants/tenant-a/members/alice/access-plan');
  expect(fetchMock.mock.calls[0][1].method).toBeUndefined();
});
it('clears previous evidence when refresh fails rather than showing old access as current', async () => {
  fetchMock.mockResolvedValueOnce({ ok: true, json: async () => ({ data: report }) });
  fetchMock.mockResolvedValueOnce({ ok: false, json: async () => ({ error: { code: 'UNAVAILABLE' } }) });
  renderPanel();
  await screen.findByText('project-owner');
  fireEvent.click(screen.getByText('다시 확인'));
  await screen.findByRole('alert');
  expect(screen.queryByText('project-owner')).not.toBeInTheDocument();
});
it('uses Japanese report and source labels', async () => {
  localStorage.setItem('nabiman_lang', 'ja');
  fetchMock.mockResolvedValue({ ok: true, json: async () => ({ data: report }) });
  renderPanel();
  await screen.findByText('外部グループ所属');
  expect(screen.getByText('差異または未確認項目あり')).toBeInTheDocument();
});
it('refreshes parent membership evidence after inspection completes', async () => {
  fetchMock.mockResolvedValue({ ok: true, json: async () => ({ data: report }) });
  const onInspected = jest.fn();
  render(<I18nProvider><TenantAccessInspection tenantId="tenant-a" userId="alice" onClose={() => {}} onInspected={onInspected} /></I18nProvider>);
  await screen.findByText('project-owner');
  expect(onInspected).toHaveBeenCalledTimes(1);
});

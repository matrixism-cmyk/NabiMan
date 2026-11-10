import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { useApi } from '../../shared/api/useApi';
import AboutDialog from './AboutDialog';
import { buildDiagnostics, contactHref, licenseSummary, toolsSummary } from './aboutInfo';

jest.mock('../../shared/api/useApi', () => ({ useApi: jest.fn(), apiRequest: jest.fn() }));

const runtime = {
  package_version: '0.1.0',
  git_commit: '9cdcaeb1234',
  tools: { checked_at: '2026-11-11T00:00:00Z', tools: [
    { name: 'tmux', found: true, version: 'tmux 3.8', features: ['terminal'] },
    { name: 'guacd', found: false, version: null, features: ['rdp'] },
    { name: 'ffmpeg', found: false, version: null, features: [] },
  ] },
};
const license = { active: true, tier: 'pro', is_trial: false, trial_days_remaining: null, expires_at: '2027-01-01', validation_state: 'licensed' };

function mockApi() {
  (useApi as jest.Mock).mockImplementation((endpoint: string) => ({
    data: endpoint === '/api/license' ? license : runtime, loading: false, error: null, refetch: jest.fn(),
  }));
}

describe('aboutInfo', () => {
  it('summarises tools, the license and contacts', () => {
    expect(toolsSummary(runtime)).toBe('found: tmux tmux 3.8; missing: guacd (rdp)');
    expect(toolsSummary(null)).toBe('not checked');
    expect(licenseSummary(license)).toBe('pro, active, expires 2027-01-01, licensed');
    expect(licenseSummary({ ...license, is_trial: true, trial_days_remaining: 4, expires_at: null, validation_state: undefined }))
      .toBe('pro, active, trial 4d left');
    expect(contactHref('noc@example.com')).toBe('mailto:noc@example.com');
    expect(contactHref('https://help.example.com/x')).toBe('https://help.example.com/x');
    expect(contactHref('02-1234-5678')).toBeNull();
  });

  it('builds diagnostics without the hash or anything that looks like a token', () => {
    window.history.replaceState(null, '', '/share/abcdef#remote/remote?token=eyJa.b.c');
    const text = buildDiagnostics({ product: 'NabiMan', runtime, license, lang: 'en', theme: 'light' }, new Date('2026-11-11T00:00:00Z'));
    expect(text).toContain('Version: 0.1.0');
    expect(text).toContain('Commit: 9cdcaeb1234');
    expect(text).toContain('Theme: light');
    expect(text).toContain('Tools: found: tmux');
    expect(text).not.toContain('abcdef');
    expect(text).not.toContain('eyJ');
    expect(text).not.toContain('#remote');
    window.history.replaceState(null, '', '/');
  });
});

describe('AboutDialog', () => {
  beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); });

  it('shows version, commit, license, notices and the support contact', () => {
    mockApi();
    render(<I18nProvider><AboutDialog onClose={() => {}} branding={{ product_name: 'Acme Ops', feedback_widget: null, noc_contact: 'noc@acme.test' }} /></I18nProvider>);
    expect(screen.getByRole('dialog', { name: 'Acme Ops 정보' })).toBeInTheDocument();
    expect(screen.getByText('0.1.0')).toBeInTheDocument();
    expect(screen.getByText('9cdcaeb1234')).toBeInTheDocument();
    expect(screen.getByText(/PRO · 활성/)).toBeInTheDocument();
    expect(screen.getByText(/THIRD_PARTY_LICENSES\.md/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'noc@acme.test' })).toHaveAttribute('href', 'mailto:noc@acme.test');
    // No licence screen for this account: no button to it.
    expect(screen.queryByRole('button', { name: '라이선스 화면 열기' })).not.toBeInTheDocument();
  });

  it('opens the license screen and copies diagnostics', async () => {
    mockApi();
    const writeText = jest.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    const onClose = jest.fn();
    const onOpenLicense = jest.fn();
    render(<I18nProvider><AboutDialog onClose={onClose} branding={null} onOpenLicense={onOpenLicense} /></I18nProvider>);
    expect(screen.getByText('등록된 연락처가 없습니다')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '진단 정보 복사' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('진단 정보를 복사했습니다.'));
    expect(writeText.mock.calls[0][0]).toContain('Commit: 9cdcaeb1234');
    fireEvent.click(screen.getByRole('button', { name: '라이선스 화면 열기' }));
    expect(onClose).toHaveBeenCalled();
    expect(onOpenLicense).toHaveBeenCalled();
  });
});

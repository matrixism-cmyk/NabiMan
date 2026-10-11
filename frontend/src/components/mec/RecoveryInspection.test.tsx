import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { fetchWithRefresh } from '../../hooks/useApi';
import RecoveryInspection, { RecoveryReport } from './RecoveryInspection';
jest.mock('../../hooks/useApi', () => ({ fetchWithRefresh: jest.fn() }));
const request = fetchWithRefresh as jest.Mock;
const report: RecoveryReport = { checked_at: '2026-10-05T00:00:00Z', configured_mode: 'real',
  records_conflict: false, profile: null, job_snapshot: null,
  observations: [{ resource: 'namespace', target: 'test-namespace', state: 'unknown',
    expected_identity_matches: null, candidates: [], error_code: 'UNAVAILABLE' },
    { resource: 'project', target: null, state: 'candidates', expected_identity_matches: null,
      candidates: ['candidate-project'], error_code: null }] };
const response = (data: RecoveryReport) => ({ ok: true, json: async () => ({ data }) });
beforeEach(() => { request.mockReset(); localStorage.setItem('nabiman_lang', 'ko'); });
const setup = () => render(<I18nProvider><RecoveryInspection jobId="job-test" /></I18nProvider>);
it('distinguishes unavailable sources from absence and candidates from ownership', async () => {
  request.mockResolvedValue(response(report)); setup();
  await screen.findByText('확인 실패');
  expect(screen.queryByText('조회 대상에 없음')).not.toBeInTheDocument();
  expect(screen.getByText('후보 발견 · 소유권 확인 필요')).toBeInTheDocument();
  expect(screen.getByText(/candidate-project/)).toBeInTheDocument();
  expect(screen.getByText(/소유권 증명이 아닙니다/)).toBeInTheDocument();
});
it('labels demo evidence and changed resource records', async () => {
  request.mockResolvedValue(response({ ...report, configured_mode: 'mock', records_conflict: true })); setup();
  await screen.findByText(/데모 데이터입니다/);
  expect(screen.getByText(/현재 테넌트 기록과 작업 당시 자원 ID가 다릅니다/)).toBeInTheDocument();
});
it('removes prior evidence when refreshing fails', async () => {
  request.mockResolvedValueOnce(response(report));
  request.mockResolvedValueOnce({ ok: false, status: 503, json: async () => ({ error: { message: 'Source unavailable' } }) });
  setup(); await screen.findByText('test-namespace');
  fireEvent.click(screen.getByRole('button', { name: '새로고침' }));
  await screen.findByText('Source unavailable');
  expect(screen.queryByText('test-namespace')).not.toBeInTheDocument();
  expect(screen.queryByText('확인 실패')).not.toBeInTheDocument();
});

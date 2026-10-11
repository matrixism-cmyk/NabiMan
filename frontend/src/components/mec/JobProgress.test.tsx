import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { TextDecoder } from 'util';
import { I18nProvider } from '../../i18n';
import JobProgress from './JobProgress';

Object.defineProperty(global, 'TextDecoder', { value: TextDecoder });
const originalFetch = global.fetch;
afterEach(() => { global.fetch = originalFetch; });
beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  sessionStorage.setItem('nabiman_token', 'test-session');
});

it.each(['completed', 'failed', 'unknown_result'])('restores %s history from persisted snapshot and closes the stream', async (state) => {
  const reason = state === 'unknown_result' ? 'Verify external resources before retrying' : undefined;
  const snapshot = { event: 'snapshot', job_id: 'job-history', status: { state, reason },
    steps: [{ name: 'create_namespace', status: 'completed', duration_ms: 11 },
      { name: 'apply_quota', status: state === 'completed' ? 'completed' : 'pending' }] };
  const read = jest.fn().mockResolvedValueOnce({ done: false,
    value: Buffer.from(`event: snapshot\ndata: ${JSON.stringify(snapshot)}\n\n`) });
  const fetchMock = jest.fn().mockResolvedValue({ ok: true, body: { getReader: () => ({ read }) } });
  global.fetch = fetchMock;
  const view = render(<I18nProvider><JobProgress jobId="job-history" /></I18nProvider>);
  await screen.findByText('create_namespace');
  expect(screen.getByText('apply_quota')).toBeInTheDocument();
  if (state === 'unknown_result') {
    expect(screen.getByText('결과 확인 필요')).toBeInTheDocument();
    expect(screen.getByText(/Verify external resources/)).toBeInTheDocument();
  } else {
    expect(screen.getByText(state)).toBeInTheDocument();
  }
  await waitFor(() => expect(fetchMock.mock.calls[0][1].signal.aborted).toBe(true));
  expect(read).toHaveBeenCalledTimes(1);
  expect(fetchMock.mock.calls[0][0]).toBe('/api/mec/v1/jobs/job-history/stream');
  expect(fetchMock.mock.calls[0][1].headers.Authorization).toBe('Bearer test-session');
  const bar = view.container.querySelector('div[style*="transition"]') as HTMLElement;
  expect(bar.style.width).toBe(state === 'completed' ? '100%' : '50%');
});

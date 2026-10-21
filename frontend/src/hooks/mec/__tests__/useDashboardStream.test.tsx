import { renderHook, waitFor } from '@testing-library/react';
import { fetchWithRefresh } from '../../../shared/api/useApi';
import { useDashboardStream } from '../useDashboardStream';
jest.mock('../../../shared/api/useApi', () => ({ fetchWithRefresh: jest.fn() }));
it('uses the refresh-aware header client without a query credential', async () => {
  sessionStorage.setItem('nabiman_token', 'never-stream-in-url');
  (fetchWithRefresh as jest.Mock).mockResolvedValue({ ok: false, status: 503 });
  const hook = renderHook(() => useDashboardStream('/api/mec/v1/dashboard/live/stream'));
  await waitFor(() => expect(hook.result.current.error).toBe('HTTP 503'));
  expect(fetchWithRefresh).toHaveBeenCalledWith('/api/mec/v1/dashboard/live/stream', expect.objectContaining({ signal: expect.any(AbortSignal) }));
  hook.unmount();
});

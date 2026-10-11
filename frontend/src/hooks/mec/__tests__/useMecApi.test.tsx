import { act, renderHook, waitFor } from '@testing-library/react';
import { fetchWithRefresh } from '../../useApi';
import { mecList, useMecList } from '../useMecApi';

jest.mock('../../useApi', () => ({ fetchWithRefresh: jest.fn() }));
const fetchMock = fetchWithRefresh as jest.Mock;
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status });

it('clears previously loaded records and reports a structured 404 instead of an empty inventory', async () => {
  fetchMock.mockResolvedValueOnce(response({ data: [{ id: 'owned-binding' }], meta: {} }));
  const hook = renderHook(() => useMecList<{ id: string }>('/api/mec/v1/tenants/example/scoped-managed-memberships'));
  await waitFor(() => expect(hook.result.current.data).toEqual([{ id: 'owned-binding' }]));
  fetchMock.mockResolvedValueOnce(response({ error: { code: 'not_found', message: 'Tenant unavailable' } }, 404));
  await act(async () => { await hook.result.current.refetch(); });
  expect(hook.result.current.error).toBe('Tenant unavailable');
  expect(hook.result.current.data).toBeNull();
  hook.unmount();
});

it.each([{}, { data: null }, { data: {} }])('rejects malformed inventory rather than inferring absence: %j', async body => {
  fetchMock.mockResolvedValueOnce(response(body));
  await expect(mecList('/inventory')).rejects.toThrow('Invalid MEC list response');
});

it('rejects an error envelope even when it includes an empty data array', async () => {
  fetchMock.mockResolvedValueOnce(response({ data: [], error: { code: 'upstream', message: 'Inventory incomplete' } }));
  await expect(mecList('/inventory')).rejects.toThrow('Inventory incomplete');
});

it('accepts a successful empty inventory and recovers from a failed refresh', async () => {
  fetchMock.mockResolvedValueOnce(response({ error: { code: 'unavailable', message: 'Unavailable' } }, 404));
  const hook = renderHook(() => useMecList('/inventory'));
  await waitFor(() => expect(hook.result.current.error).toBe('Unavailable'));
  fetchMock.mockResolvedValueOnce(response({ data: [], meta: {} }));
  await act(async () => { await hook.result.current.refetch(); });
  expect(hook.result.current.data).toEqual([]);
  expect(hook.result.current.error).toBeNull();
  hook.unmount();
});

import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import { RemoteServer } from './remoteTypes';
import ProtocolDetect from './ProtocolDetect';

jest.mock('../../shared/api/useApi', () => ({ apiPost: jest.fn(), apiRequest: jest.fn() }));
const post = apiPost as jest.Mock;
const request = apiRequest as jest.Mock;

const server: RemoteServer = {
  id: 'srv_1', name: 'files', host: 'h', port: 22, user: 'u', auth_method: 'key',
  tags: [], memo: '', created_at: '', last_checked: '', status: '',
  host_trust: { public_key: 'k', fingerprint: 'f', verified_by: 'a', verified_at: '', reason: 'r' },
};

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  sessionStorage.clear();
  post.mockReset();
  request.mockReset();
});

const ui = (onChanged = jest.fn()) => render(<I18nProvider><ProtocolDetect server={server} onChanged={onChanged} /></I18nProvider>);

test('a refused shell with working SFTP makes the server SFTP only', async () => {
  post.mockResolvedValue({ success: true, data: {
    checks: [{ protocol: 'ssh', ok: false, message: 'Shell refused' }, { protocol: 'sftp', ok: true, message: 'SFTP available' }],
    terminal: 'none', files: 'sftp', changed: true,
  } });
  request.mockResolvedValue({ success: true, data: server });
  const onChanged = jest.fn();
  ui(onChanged);
  fireEvent.click(screen.getByRole('button', { name: '확인하기' }));
  await waitFor(() => expect(onChanged).toHaveBeenCalled());
  expect(request).toHaveBeenCalledWith('/api/remote-servers/srv_1', expect.objectContaining({
    method: 'PUT', body: JSON.stringify({ terminal: 'none', files: 'sftp' }),
  }));
  expect(screen.getByRole('status')).toHaveTextContent('SFTP 전용');
  expect(screen.getByText('Shell refused')).toBeInTheDocument();
});

test('when nothing works the settings are left alone', async () => {
  post.mockResolvedValue({ success: true, data: {
    checks: [{ protocol: 'ssh', ok: false, message: 'Connection refused' }, { protocol: 'sftp', ok: false, message: 'refused' }],
    terminal: 'ssh', files: 'sftp', changed: false,
  } });
  ui();
  fireEvent.click(screen.getByRole('button', { name: '확인하기' }));
  expect(await screen.findByRole('status')).toHaveTextContent('설정을 그대로 두었습니다');
  expect(request).not.toHaveBeenCalled();
});

test('a server just added is checked once, by itself', async () => {
  sessionStorage.setItem('nabiman_detect_pending', 'srv_1');
  post.mockResolvedValue({ success: true, data: { checks: [{ protocol: 'ssh', ok: true, message: '' }], terminal: 'ssh', files: 'sftp', changed: false } });
  ui();
  await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
  expect(sessionStorage.getItem('nabiman_detect_pending')).toBeNull();
});

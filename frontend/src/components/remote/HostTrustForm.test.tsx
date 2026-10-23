import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import HostTrustForm from './HostTrustForm';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import { RemoteServer } from './remoteTypes';
jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn(), apiPost: jest.fn() }));
const request = apiRequest as jest.Mock;
const post = apiPost as jest.Mock;
const server: RemoteServer = { id: 'srv_test', name: 'Test', host: 'example.test', port: 22,
  user: 'test', auth_method: 'key', tags: [], memo: '', status: '', last_checked: '', created_at: '',
  host_trust_version: 'approval-before', host_trust: { public_key: 'ssh-ed25519 OLD', fingerprint: 'SHA256:old', verified_by: 'admin', verified_at: '', reason: 'console' } };
const setup = () => {
  const saved = jest.fn();
  render(<I18nProvider><HostTrustForm server={server} onSaved={saved} /></I18nProvider>);
  return saved;
};
beforeEach(() => { request.mockReset(); post.mockReset(); localStorage.setItem('nabiman_lang', 'ko'); });
it('requires verification reason before submitting', () => {
  setup(); fireEvent.click(screen.getByRole('button', { name: '확인한 키 등록 또는 교체' }));
  expect(request).not.toHaveBeenCalled();
});
it('preserves previous fingerprint when replacing and shows conflict', async () => {
  const saved = setup(); request.mockResolvedValue({ success: false, message: 'Host trust changed; reload' });
  fireEvent.change(screen.getByLabelText('서버 공개키'), { target: { value: 'ssh-ed25519 NEW' } });
  fireEvent.change(screen.getByLabelText('독립 확인한 SHA256 지문'), { target: { value: 'SHA256:new' } });
  fireEvent.change(screen.getByLabelText('확인 경로와 변경 사유'), { target: { value: 'console verified rotation' } });
  fireEvent.click(screen.getByRole('button', { name: '확인한 키 등록 또는 교체' }));
  await screen.findByText('Host trust changed; reload');
  expect(JSON.parse(request.mock.calls[0][1].body)).toEqual({ host: 'example.test', port: 22, public_key: 'ssh-ed25519 NEW', fingerprint: 'SHA256:new', previous_fingerprint: 'SHA256:old', previous_version: 'approval-before', reason: 'console verified rotation', legacy: false });
  expect(saved).not.toHaveBeenCalled();
});
it('revokes trust with an explicit reason and refreshes server', async () => {
  const saved = setup(); request.mockResolvedValue({ success: true });
  fireEvent.change(screen.getByLabelText('확인 경로와 변경 사유'), { target: { value: 'retired host key' } });
  fireEvent.click(screen.getByRole('button', { name: '신뢰 해제' }));
  await waitFor(() => expect(saved).toHaveBeenCalled());
  expect(JSON.parse(request.mock.calls[0][1].body)).toEqual({ host: 'example.test', port: 22, public_key: '', fingerprint: '', previous_fingerprint: 'SHA256:old', previous_version: 'approval-before', reason: 'retired host key', legacy: false });
});
it('fills in the key a server not yet trusted presents, and registers it only when asked', async () => {
  post.mockResolvedValue({ success: true, data: { public_key: 'ssh-ed25519 FETCHED', fingerprint: 'SHA256:fetched' } });
  render(<I18nProvider><HostTrustForm server={{ ...server, host_trust: null }} onSaved={jest.fn()} /></I18nProvider>);
  await screen.findByText(/SHA256:fetched 이\(가\)/);
  expect(post).toHaveBeenCalledWith('/api/remote-servers/srv_test/host-key', {});
  expect(screen.getByLabelText('서버 공개키')).toHaveValue('ssh-ed25519 FETCHED');
  expect(screen.getByLabelText('독립 확인한 SHA256 지문')).toHaveValue('SHA256:fetched');
  expect(request).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '확인한 키 등록 또는 교체' }));
  await waitFor(() => expect(request).toHaveBeenCalled());
  expect(JSON.parse(request.mock.calls[0][1].body)).toMatchObject({ public_key: 'ssh-ed25519 FETCHED', fingerprint: 'SHA256:fetched', reason: '서버에서 직접 가져온 키' });
});
it('does not fetch on its own for a server already trusted', () => {
  setup();
  expect(post).not.toHaveBeenCalled();
});

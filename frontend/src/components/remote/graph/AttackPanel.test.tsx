import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../../i18n';
import AttackPanel from './AttackPanel';

const mockApi = jest.fn();
jest.mock('../../../shared/api/useApi', () => ({ apiRequest: (...a: unknown[]) => mockApi(...a) }));
const apiRequest = mockApi;

const report = {
  readable: true, source: 'journal', window_min: 60, failed: 7, kinds: { failed_password: 5, invalid_user: 2 }, sources: 2, per_minute: [0, 3, 4],
  attackers: [
    { ip: '192.236.156.181', count: 6, last: 1800000000, users: ['root', 'admin'], internal: false, banned: true },
    { ip: '172.20.26.200', count: 1, last: 1800000000, users: ['jcia'], internal: true, banned: false },
  ],
  accepted: [{ ip: '172.20.26.200', user: 'jcia', at: 1800000000, method: 'password' }], banned: ['192.236.156.181'], checked_at: 1800000000,
};

beforeEach(() => { localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); apiRequest.mockReset(); });

it('lists who tried to log in, how often and as whom', async () => {
  apiRequest.mockResolvedValue({ success: true, data: report });
  render(<I18nProvider><AttackPanel id="srv_1" name="spark" sub="spark.bebo.kr" onClose={jest.fn()} /></I18nProvider>);
  await waitFor(() => expect(screen.getByText('192.236.156.181')).toBeInTheDocument());
  expect(apiRequest).toHaveBeenCalledWith('/api/remote-servers/srv_1/attacks?window=60');
  expect(screen.getByText('root, admin')).toBeInTheDocument();
  expect(screen.getByText('차단됨')).toBeInTheDocument();
  expect(screen.getByText('내부')).toBeInTheDocument();
  expect(screen.getByText('성공한 로그인')).toBeInTheDocument();
});

it('says so when the log cannot be read, and when the machine is not saved', async () => {
  apiRequest.mockResolvedValue({ success: true, data: { ...report, readable: false, failed: 0, attackers: [], accepted: [], source: 'none' } });
  const { unmount } = render(<I18nProvider><AttackPanel id="srv_1" name="x" onClose={jest.fn()} /></I18nProvider>);
  await waitFor(() => expect(screen.getByText(/SSH 로그가 없습니다/)).toBeInTheDocument());
  unmount();
  apiRequest.mockResolvedValue({ success: true, data: { ...report, readable: false, failed: 0, attackers: [], accepted: [], source: '/var/log/secure' } });
  const again = render(<I18nProvider><AttackPanel id="srv_2" name="x" onClose={jest.fn()} /></I18nProvider>);
  await waitFor(() => expect(screen.getByText(/SSH 로그를 읽을 수 없습니다/)).toBeInTheDocument());
  again.unmount();
  apiRequest.mockClear();
  render(<I18nProvider><AttackPanel id={null} name="y" onClose={jest.fn()} /></I18nProvider>);
  expect(screen.getByText(/저장된 서버가 아니어서/)).toBeInTheDocument();
  expect(apiRequest).not.toHaveBeenCalled();
});

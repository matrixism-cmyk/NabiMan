import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { apiPost } from '../../shared/api/useApi';
import ServerForm from './ServerForm';

jest.mock('../../shared/api/useApi', () => ({ apiPost: jest.fn(), apiRequest: jest.fn() }));
const post = apiPost as jest.Mock;

beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); post.mockReset(); });

const setup = (onSaved = jest.fn()) => {
  const view = render(<I18nProvider><ServerForm onSaved={onSaved} /></I18nProvider>);
  const form = view.container.querySelector('form') as HTMLFormElement;
  return { ...view, form, onSaved };
};

test('an FTP-only server asks for the FTP port and a password, not SSH', () => {
  const { container } = setup();
  fireEvent.change(screen.getByLabelText('터미널 접속'), { target: { value: 'none' } });
  fireEvent.change(screen.getByLabelText('파일 접속'), { target: { value: 'ftp' } });
  expect(screen.queryByLabelText('SSH 포트')).not.toBeInTheDocument();
  expect(screen.getByLabelText('FTP 포트')).toHaveValue(21);
  expect(screen.getByRole('note')).toHaveTextContent('FTP');
  expect(container.querySelector('option[value="key"]')).toBeNull();
  expect(container.querySelector('input[type="password"]')).not.toBeNull();
});

test('saving sends both protocols and logs in with a password', async () => {
  post.mockResolvedValue({ success: true, data: { id: 'srv_1', host_trust: null } });
  const { container, form, onSaved } = setup();
  fireEvent.change(container.querySelector('input[placeholder="192.168.1.100"]')!, { target: { value: 'files.example.com' } });
  fireEvent.change(screen.getByLabelText('터미널 접속'), { target: { value: 'none' } });
  fireEvent.change(screen.getByLabelText('파일 접속'), { target: { value: 'ftps' } });
  fireEvent.change(screen.getByLabelText('FTP 포트'), { target: { value: '990' } });
  fireEvent.change(screen.getByLabelText('FTPS 인증서 지문 (선택)'), { target: { value: ' SHA256:AB ' } });
  fireEvent.change(container.querySelector('input[placeholder="root"]')!, { target: { value: 'web@example.com' } });
  fireEvent.submit(form);
  await waitFor(() => expect(onSaved).toHaveBeenCalled());
  expect(post).toHaveBeenCalledWith('/api/remote-servers', expect.objectContaining({
    terminal: 'none', files: 'ftps', ftp_port: 990, tls_fingerprint: 'SHA256:AB',
    auth_method: 'password', user: 'web@example.com',
  }));
  // The new server is looked at once its settings open.
  expect(sessionStorage.getItem('nabiman_detect_pending')).toBe('srv_1');
});

test('Telnet takes port 23 and cannot carry SFTP', () => {
  const { form } = setup();
  fireEvent.change(screen.getByLabelText('터미널 접속'), { target: { value: 'telnet' } });
  expect(screen.getByLabelText('Telnet 포트')).toHaveValue(23);
  fireEvent.change(screen.getByLabelText('터미널 접속'), { target: { value: 'ssh' } });
  expect(screen.getByLabelText('SSH 포트')).toHaveValue(22);
  fireEvent.change(screen.getByLabelText('터미널 접속'), { target: { value: 'telnet' } });
  fireEvent.change(form.querySelector('input[placeholder="192.168.1.100"]')!, { target: { value: 'router' } });
  fireEvent.submit(form);
  expect(screen.getByText(/SFTP는 SSH 위에서 동작합니다/)).toBeInTheDocument();
  expect(post).not.toHaveBeenCalled();
});

test('a server with neither terminal nor files is refused', () => {
  const { form } = setup();
  fireEvent.change(form.querySelector('input[placeholder="192.168.1.100"]')!, { target: { value: 'h' } });
  fireEvent.change(screen.getByLabelText('터미널 접속'), { target: { value: 'none' } });
  fireEvent.change(screen.getByLabelText('파일 접속'), { target: { value: 'none' } });
  fireEvent.submit(form);
  expect(screen.getByText('터미널과 파일 중 하나는 골라야 합니다.')).toBeInTheDocument();
});

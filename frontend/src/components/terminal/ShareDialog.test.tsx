import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { apiPost, apiRequest } from '../../shared/api/useApi';
import ShareDialog from './ShareDialog';

jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn(), apiPost: jest.fn() }));
const link = { token: 'test-token', session_id: 'session-1', expires_unix: 0, read_only: true, needs_password: false };
const request = apiRequest as jest.Mock;
const post = apiPost as jest.Mock;
const setup = (onClose = jest.fn()) => render(<I18nProvider><ShareDialog sessionId="session-1" sessionLabel="Test shell" onClose={onClose} /></I18nProvider>);

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  window.confirm = jest.fn(() => true);
  request.mockResolvedValue({ success: true, data: [link] });
});

test('failed deletion keeps the link and displays the error', async () => {
  request.mockResolvedValueOnce({ success: true, data: [link] }).mockResolvedValueOnce({ success: false, message: '삭제 실패' });
  setup();
  fireEvent.click(await screen.findByRole('button', { name: '삭제' }));
  expect(await screen.findByText('삭제 실패')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '삭제' })).toBeInTheDocument();
  expect(request).toHaveBeenCalledTimes(2);
});

test('new link appears once, with a delete action', async () => {
  request.mockResolvedValueOnce({ success: true, data: [] });
  post.mockResolvedValue({ success: true, data: link });
  setup();
  await waitFor(() => expect(request).toHaveBeenCalled());
  fireEvent.click(screen.getByRole('button', { name: '링크 만들기' }));
  await screen.findByRole('button', { name: '삭제' });
  expect(screen.getAllByDisplayValue('http://localhost/share/test-token')).toHaveLength(1);
  expect(document.querySelectorAll('.share-created')).toHaveLength(0);
});

test('Escape closes the dialog even after focus leaves a removed control', async () => {
  const close = jest.fn();
  setup(close);
  await screen.findByRole('button', { name: '삭제' });
  fireEvent.keyDown(document.body, { key: 'Escape' });
  expect(close).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('dialog').closest('.term-window')).toBeNull();
});

test('defaults to read-only, offers finite expiry and identifies links requiring reissue', async () => {
  request.mockResolvedValue({ success: true, data: [{ ...link, usable: false }] });
  setup();
  expect(screen.getByRole('checkbox')).toBeChecked();
  expect(screen.queryByRole('button', { name: '영구' })).not.toBeInTheDocument();
  expect(await screen.findByText('사용 불가 · 필요 시 재발급')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '복사' })).toBeDisabled();
});

test('file access is off unless chosen, and the choice is sent with the link', async () => {
  request.mockResolvedValueOnce({ success: true, data: [] });
  post.mockResolvedValue({ success: true, data: { ...link, files: 'write' } });
  setup();
  await waitFor(() => expect(request).toHaveBeenCalled());
  expect(screen.getByRole('radio', { name: '끄기' })).toHaveAttribute('aria-checked', 'true');
  fireEvent.click(screen.getByRole('radio', { name: '올리기·수정까지' }));
  fireEvent.click(screen.getByRole('button', { name: '링크 만들기' }));
  await waitFor(() => expect(post).toHaveBeenCalledWith('/api/terminal/sessions/session-1/share-link',
    expect.objectContaining({ files: 'write', read_only: true })));
});

test('a files link goes to the server, needs file access, and lists only its own links', async () => {
  const files = { ...link, token: 'files-token', session_id: '', server_id: 'srv_ftp', files: 'read' };
  request.mockResolvedValue({ success: true, data: [link, files] });
  post.mockResolvedValue({ success: true, data: files });
  render(<I18nProvider><ShareDialog serverId="srv_ftp" sessionLabel="FTP files" onClose={jest.fn()} /></I18nProvider>);
  expect(await screen.findByRole('heading', { name: '파일 공유' })).toBeInTheDocument();
  await waitFor(() => expect(screen.getAllByRole('button', { name: '삭제' })).toHaveLength(1));
  expect(screen.queryByRole('radio', { name: '끄기' })).not.toBeInTheDocument();
  expect(screen.queryByText('읽기 전용')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('radio', { name: '올리기·수정까지' }));
  fireEvent.click(screen.getByRole('button', { name: '링크 만들기' }));
  await waitFor(() => expect(post).toHaveBeenCalledWith('/api/terminal/files/srv_ftp/share-link', expect.objectContaining({ files: 'write' })));
});

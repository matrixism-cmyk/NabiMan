import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { apiPost } from '../../hooks/useApi';
import SessionNameEdit from './SessionNameEdit';

jest.mock('../../hooks/useApi', () => ({ apiPost: jest.fn() }));
const post = apiPost as jest.Mock;

const setup = (name = '', onRenamed = jest.fn()) =>
  render(<I18nProvider><SessionNameEdit sessionId="pty_1" name={name} onRenamed={onRenamed} /></I18nProvider>);

beforeEach(() => localStorage.setItem('nabiman_lang', 'ko'));

test('an unnamed session says so, and the pencil opens the name for editing', () => {
  setup();
  expect(screen.getByText('이름 없는 세션')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '이름 바꾸기' }));
  expect(screen.getByRole('textbox', { name: '세션 이름' })).toHaveValue('');
});

test('saving sends the name and hands the saved one back', async () => {
  post.mockResolvedValue({ success: true, data: '배포용 셸' });
  const renamed = jest.fn();
  setup('old', renamed);
  fireEvent.click(screen.getByRole('button', { name: '이름 바꾸기' }));
  const input = screen.getByRole('textbox', { name: '세션 이름' });
  expect(input).toHaveValue('old');
  fireEvent.change(input, { target: { value: ' 배포용 셸 ' } });
  fireEvent.submit(input);
  await waitFor(() => expect(renamed).toHaveBeenCalledWith('배포용 셸'));
  expect(post).toHaveBeenCalledWith('/api/terminal/sessions/pty_1/name', { name: ' 배포용 셸 ' });
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
});

test('a refused rename stays open with the reason', async () => {
  post.mockResolvedValue({ success: false, message: 'Session not found' });
  const renamed = jest.fn();
  setup('old', renamed);
  fireEvent.click(screen.getByRole('button', { name: '이름 바꾸기' }));
  fireEvent.click(screen.getByRole('button', { name: '저장' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('Session not found');
  expect(screen.getByRole('textbox', { name: '세션 이름' })).toBeInTheDocument();
  expect(renamed).not.toHaveBeenCalled();
});

test('Escape gives up without saving', () => {
  setup('old');
  fireEvent.click(screen.getByRole('button', { name: '이름 바꾸기' }));
  fireEvent.keyDown(screen.getByRole('textbox', { name: '세션 이름' }), { key: 'Escape' });
  expect(screen.getByText('old')).toBeInTheDocument();
  expect(post).not.toHaveBeenCalled();
});

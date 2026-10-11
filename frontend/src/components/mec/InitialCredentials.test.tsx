import React, { useState } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import InitialCredentials, { randomPassword } from './InitialCredentials';
function Receipt() {
  const [visible, setVisible] = useState(true);
  return visible ? <InitialCredentials credentials={{ username: 'test-user', password: 'ephemeral-fixture-password' }} onDone={() => setVisible(false)} /> : <p>Closed</p>;
}
it('masks the initial secret and removes it after the explicit acknowledgement', () => {
  localStorage.setItem('nabiman_lang', 'ko');
  render(<I18nProvider><Receipt /></I18nProvider>);
  const input = screen.getByLabelText('초기 비밀번호');
  expect(input).toHaveAttribute('type', 'password');
  fireEvent.click(screen.getByRole('button', { name: '비밀번호 보기' }));
  expect(input).toHaveAttribute('type', 'text');
  fireEvent.click(screen.getByRole('button', { name: '안전하게 보관했습니다' }));
  expect(screen.queryByLabelText('초기 비밀번호')).not.toBeInTheDocument();
});
it('uses browser cryptographic random values without Math.random fallback', () => {
  const insecure = jest.spyOn(Math, 'random').mockImplementation(() => { throw new Error('Insecure random source used'); });
  try { const first = randomPassword(); const second = randomPassword(); expect(first).toHaveLength(24); expect(first).toMatch(/^[A-Za-z0-9_-]+$/); expect(first).not.toBe(second); } finally { insecure.mockRestore(); }
});

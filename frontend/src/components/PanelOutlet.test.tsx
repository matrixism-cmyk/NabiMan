import React from 'react';
import { render, screen } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import PanelOutlet from './PanelOutlet';

jest.mock('./SwapPanel', () => ({ __esModule: true, default: () => <p>swap panel</p> }));
jest.mock('./DnsPanel', () => ({ __esModule: true, default: () => { throw new Error('dns panel broke'); } }));

beforeEach(() => {
  localStorage.setItem('nabiman_lang', 'ko');
  jest.spyOn(console, 'error').mockImplementation(() => undefined);
});
afterEach(() => (console.error as jest.Mock).mockRestore());

const outlet = (tab: 'swap' | 'dns') => (
  <I18nProvider><PanelOutlet activeTab={tab} onNavigate={() => {}} onEnterFocus={() => {}} /></I18nProvider>
);

test('a lazily loaded panel shows a short loading line, then itself', async () => {
  render(outlet('swap'));
  expect(screen.getByRole('status')).toHaveTextContent('화면을 불러오는 중');
  expect(await screen.findByText('swap panel')).toBeInTheDocument();
});

test('a panel that throws is caught in its own boundary, and the next tab starts clean', async () => {
  const { rerender } = render(outlet('dns'));
  expect(await screen.findByRole('alert')).toHaveTextContent('이 부분을 표시하지 못했습니다');
  expect(screen.getByText(/dns panel broke/)).toBeInTheDocument();
  rerender(outlet('swap'));
  expect(await screen.findByText('swap panel')).toBeInTheDocument();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

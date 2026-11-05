import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../i18n';
import { apiRequest } from '../../shared/api/useApi';
import ClaudeAddress, { ClaudePeer, useClaudePeers } from './ClaudeAddress';

jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
const request = apiRequest as jest.Mock;

const peer: ClaudePeer = { name: 'artisland-bd', ref: '946528', address: 'artisland-bd [946528]', cwd: '/workspace/artisland', busy: true };

beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); request.mockReset(); });

test('shows the address as ListAgents does and copies it whole', async () => {
  const writeText = jest.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  Object.defineProperty(window, 'isSecureContext', { value: true, configurable: true });
  render(<I18nProvider><ClaudeAddress peers={[peer]} /></I18nProvider>);
  const chip = screen.getByRole('button', { name: 'Claude 세션 주소 artisland-bd [946528] 복사' });
  expect(chip).toHaveTextContent('artisland-bd');
  expect(chip).toHaveTextContent('[946528]');
  expect(chip).toHaveTextContent('/workspace/artisland');
  expect(chip.title).toContain('작업 중');
  fireEvent.click(chip);
  await waitFor(() => expect(writeText).toHaveBeenCalledWith('artisland-bd [946528]'));
  expect(await screen.findByText('복사됨')).toBeInTheDocument();
});

test('nothing shows when no Claude runs in the session', () => {
  const { container } = render(<I18nProvider><ClaudeAddress peers={[]} /></I18nProvider>);
  expect(container).toBeEmptyDOMElement();
});

function Probe({ id }: { id?: string }) {
  const peers = useClaudePeers(id);
  return <span>{peers.map((p) => p.address).join(',') || 'none'}</span>;
}

test('an open session asks for its Claude sessions and looks again later', async () => {
  jest.useFakeTimers();
  request.mockResolvedValue({ success: true, data: { claude: [peer], busy: true } });
  render(<Probe id="pty_1" />);
  await waitFor(() => expect(screen.getByText('artisland-bd [946528]')).toBeInTheDocument());
  expect(request).toHaveBeenCalledWith('/api/terminal/sessions/pty_1/claude');
  await act(async () => { jest.advanceTimersByTime(10000); });
  expect(request).toHaveBeenCalledTimes(2);
  jest.useRealTimers();
});

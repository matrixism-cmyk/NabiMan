import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { ShareConnection } from './terminalUrl';
import { issueShareTicket, readShareInfo } from './shareAdmission';
import SharePage from './SharePage';
let mockShare: ShareConnection | undefined;
jest.mock('./TerminalView', () => ({ __esModule: true, default: ({ share }: { share: ShareConnection }) => { mockShare = share; return <div data-testid="shared-terminal" />; } }));
jest.mock('./shareAdmission', () => {
  const actual = jest.requireActual('./shareAdmission');
  return { ...actual, issueShareTicket: jest.fn(), readShareInfo: jest.fn() };
});
const issue = issueShareTicket as jest.Mock;
const info = readShareInfo as jest.Mock;
const metadata = { label: 'Fixture shell', needs_password: true, read_only: true, alive: true, expires_unix: 12345, cols: 80, rows: 24 };
const setup = (token = 'one') => render(<I18nProvider><SharePage token={token} /></I18nProvider>);
beforeEach(() => { localStorage.clear(); sessionStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); mockShare = undefined; info.mockResolvedValue(metadata); });

it('consumes the prepared proof once and obtains fresh proof for reconnect with only a memory password', async () => {
  issue.mockResolvedValueOnce({ ticket: 'a'.repeat(64), expires_in_seconds: 30 }).mockResolvedValueOnce({ ticket: 'b'.repeat(64), expires_in_seconds: 30 });
  setup();
  fireEvent.change(await screen.findByPlaceholderText('비밀번호'), { target: { value: 'memory-only-password' } });
  fireEvent.click(screen.getByRole('button', { name: '들어가기' }));
  await screen.findByTestId('shared-terminal');
  expect(await mockShare!.acquireTicket('')).toBe('a'.repeat(64));
  expect(await mockShare!.acquireTicket('')).toBe('b'.repeat(64));
  expect(issue).toHaveBeenCalledTimes(2);
  expect(issue).toHaveBeenLastCalledWith('one', 'memory-only-password', '', undefined);
  expect(JSON.stringify(localStorage)).not.toContain('memory-only-password');
  expect(JSON.stringify(sessionStorage)).not.toContain('memory-only-password');
  expect(screen.queryByDisplayValue('memory-only-password')).not.toBeInTheDocument();
});
it('obtains admission for a passwordless share and never reuses a previous proof', async () => {
  info.mockResolvedValue({ ...metadata, needs_password: false });
  issue.mockResolvedValue({ ticket: 'a'.repeat(64), expires_in_seconds: 30 });
  setup(); await screen.findByTestId('shared-terminal');
  await mockShare!.acquireTicket('cols=80'); await mockShare!.acquireTicket('cols=80');
  expect(issue).toHaveBeenCalledTimes(2);
  expect(issue).toHaveBeenCalledWith('one', '', 'cols=80', undefined);
});
it('a different link clears authorized state and ignores a pending response from the old document', async () => {
  let complete: (value: unknown) => void = () => {};
  info.mockResolvedValueOnce({ ...metadata, needs_password: false }).mockResolvedValueOnce(metadata);
  const view = setup(); await screen.findByTestId('shared-terminal');
  issue.mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
  const original = mockShare!;
  const pending = original.acquireTicket('');
  view.rerender(<I18nProvider><SharePage token="two" /></I18nProvider>);
  await screen.findByPlaceholderText('비밀번호');
  await act(async () => { complete({ ticket: 'a'.repeat(64), expires_in_seconds: 30 }); await pending; });
  expect(screen.queryByTestId('shared-terminal')).not.toBeInTheDocument();
  await waitFor(() => expect(info).toHaveBeenLastCalledWith('two', expect.any(AbortSignal)));
});

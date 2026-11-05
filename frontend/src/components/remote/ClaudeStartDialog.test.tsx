import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import ClaudeStartDialog from './ClaudeStartDialog';
import { apiRequest } from '../../shared/api/useApi';

jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));

const options = {
  users: [{ name: 'root', home: '/root' }, { name: 'nabi', home: '/workspace/nabi' }],
  default_user: 'nabi',
  dirs: [
    { user: 'nabi', dir: '/workspace/nabiman', last_used: 2 },
    { user: 'nabi', dir: '/workspace/artisland', last_used: 1 },
    { user: 'root', dir: '/srv', last_used: null },
  ],
};

beforeEach(() => { (apiRequest as jest.Mock).mockReset().mockResolvedValue({ success: true, data: options }); });

it('suggests the latest directory and starts with the chosen options', async () => {
  const onStart = jest.fn();
  render(<ClaudeStartDialog onClose={jest.fn()} onStart={onStart} />);
  const dir = await screen.findByDisplayValue('/workspace/nabiman');
  expect(screen.getByDisplayValue('nabi (/workspace/nabi)')).toBeTruthy();
  expect(screen.getByDisplayValue('nabiman')).toBeTruthy();
  expect(screen.getByText('su nabi → cd /workspace/nabiman && claude --dangerously-skip-permissions -c')).toBeTruthy();
  fireEvent.change(dir, { target: { value: '/workspace/artisland' } });
  fireEvent.click(screen.getByLabelText(/-c\)|claudeStart\.resume/));
  fireEvent.click(screen.getByRole('button', { name: /^(시작|Start|claudeStart\.start)$/ }));
  expect(onStart).toHaveBeenCalledWith({ user: 'nabi', dir: '/workspace/artisland', skipPermissions: true, resume: false, title: 'artisland' });
});

it('switching the user offers that user\'s directories', async () => {
  render(<ClaudeStartDialog onClose={jest.fn()} onStart={jest.fn()} />);
  await screen.findByDisplayValue('/workspace/nabiman');
  fireEvent.change(screen.getByDisplayValue('nabi (/workspace/nabi)'), { target: { value: 'root' } });
  await waitFor(() => expect(screen.getByDisplayValue('/srv')).toBeTruthy());
  expect(screen.getByText('cd /srv && claude --dangerously-skip-permissions -c')).toBeTruthy();
});

it('refuses a relative directory and reports a failed load', async () => {
  const onStart = jest.fn();
  render(<ClaudeStartDialog onClose={jest.fn()} onStart={onStart} />);
  const dir = await screen.findByDisplayValue('/workspace/nabiman');
  fireEvent.change(dir, { target: { value: 'relative/path' } });
  fireEvent.click(screen.getByRole('button', { name: /^(시작|Start|claudeStart\.start)$/ }));
  expect(onStart).not.toHaveBeenCalled();
  expect(screen.getByRole('alert')).toBeTruthy();
});

it('says so when the options cannot be loaded', async () => {
  (apiRequest as jest.Mock).mockResolvedValue({ success: false, message: 'Forbidden' });
  render(<ClaudeStartDialog onClose={jest.fn()} onStart={jest.fn()} />);
  expect(await screen.findByRole('alert')).toBeTruthy();
});

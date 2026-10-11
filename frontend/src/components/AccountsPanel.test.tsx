import React from 'react';
import { fireEvent, render, screen, within, waitFor } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiDelete, apiPost, useApi } from '../hooks/useApi';
import AccountsPanel from './AccountsPanel';
jest.mock('../hooks/useApi', () => ({ useApi: jest.fn(), apiPost: jest.fn(), apiDelete: jest.fn() }));
const alice = { username: 'alice', uid: 1001, gid: 1001, home: '/home/alice', shell: '/bin/bash', is_logged_in: null, can_manage: true };
let rows = [alice]; let canWrite = true;
const view = () => <I18nProvider><AccountsPanel /></I18nProvider>;
beforeEach(() => {
  jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en'); rows = [{ ...alice }]; canWrite = true;
  (useApi as jest.Mock).mockImplementation((path: string) => ({ data: path === '/api/accounts' ? rows : { shells: ['/bin/bash'], can_write: canWrite }, refetch: jest.fn() }));
  (apiDelete as jest.Mock).mockResolvedValue({ success: true, data: 'Deleted without requesting home removal' });
  (apiPost as jest.Mock).mockResolvedValue({ success: true, data: 'Password command completed' });
});
function deletion() {
  fireEvent.click(within(screen.getByRole('row', { name: /alice/ })).getByRole('button', { name: 'Delete' }));
  const name = screen.getByLabelText('Type the account name to confirm deletion');
  return { name, form: within(name.closest('form')!) };
}
test('read-only actor cannot open mutation controls and unknown login is explicit', () => {
  canWrite = false; render(view()); expect(screen.queryByRole('button', { name: 'Add User' })).not.toBeInTheDocument();
  expect(screen.queryByRole('button', { name: 'Delete' })).not.toBeInTheDocument(); expect(screen.getByText('Login state not collected')).toBeInTheDocument();
});
test('deletion requires typed confirmation, sends observed UID and preserves home by default', async () => {
  render(view()); const d = deletion(); expect(d.form.getByRole('button', { name: 'Delete' })).toBeDisabled();
  expect(d.form.getByRole('checkbox')).not.toBeChecked(); fireEvent.change(d.name, { target: { value: 'alice' } });
  fireEvent.click(d.form.getByRole('button', { name: 'Delete' }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledWith('/api/accounts', { username: 'alice', expected_uid: 1001, remove_home: false, confirm_home: null }));
  expect(await screen.findByRole('status')).toHaveTextContent('Deleted without requesting home removal');
});
test('home deletion carries explicit selected path and a replaced UID cannot be submitted', async () => {
  const { rerender } = render(view()); const d = deletion();
  fireEvent.change(d.name, { target: { value: 'alice' } }); fireEvent.click(d.form.getByRole('checkbox'));
  rows = [{ ...alice, uid: 1002 }]; rerender(view());
  expect(screen.getByRole('alert')).toHaveTextContent('Account identity changed'); expect(d.form.getByRole('button', { name: 'Delete' })).toBeDisabled(); expect(apiDelete).not.toHaveBeenCalled();
});
test('explicit home removal is distinct from ordinary account deletion', async () => {
  render(view()); const d = deletion();fireEvent.change(d.name, { target: { value: 'alice' } });fireEvent.click(d.form.getByRole('checkbox'));fireEvent.click(d.form.getByRole('button', { name: 'Delete' }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledWith('/api/accounts', { username: 'alice', expected_uid: 1001, remove_home: true, confirm_home: '/home/alice' }));
});
test('password mutation uses selected identity and clears secrets after uncertain failure without retry', async () => {
  (apiPost as jest.Mock).mockRejectedValue(new Error('network loss'));
  render(view()); fireEvent.click(screen.getByRole('button', { name: 'Change Password' }));
  fireEvent.change(screen.getByLabelText('Select a local account'), { target: { value: 'alice' } });
  fireEvent.change(screen.getByLabelText('New Password'), { target: { value: 'fixture-password-long' } });
  fireEvent.click(screen.getByRole('button', { name: 'Change' }));
  await waitFor(() => expect(screen.getByLabelText('New Password')).toHaveValue(''));
  expect(apiPost).toHaveBeenCalledTimes(1);expect(apiPost).toHaveBeenCalledWith('/api/accounts/password', { username: 'alice', expected_uid: 1001, new_password: 'fixture-password-long' });
  expect(screen.getByRole('status')).toHaveTextContent('outcome was not confirmed');
});

import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../i18n';
import { apiPost } from '../shared/api/useApi';
import EncryptedBackupPanel from './EncryptedBackupPanel';
jest.mock('../shared/api/useApi', () => ({ apiPost: jest.fn() }));
beforeEach(() => { jest.clearAllMocks(); localStorage.setItem('nabiman_lang', 'en'); });
it('requires matching passphrases and clears both when a request fails', async () => {
 (apiPost as jest.Mock).mockRejectedValue(new Error('Storage unavailable'));
 render(<I18nProvider><EncryptedBackupPanel files={[]} refresh={jest.fn()} /></I18nProvider>);
 const button=screen.getByRole('button',{name:'Create encrypted backup'});
 fireEvent.change(screen.getByLabelText('Paths to encrypt (comma separated)'),{target:{value:'/tmp/file'}});
 fireEvent.change(screen.getByLabelText('New backup passphrase'),{target:{value:'fixture passphrase'}});
 expect(button).toBeDisabled();
 fireEvent.change(screen.getByLabelText('Confirm new backup passphrase'),{target:{value:'fixture passphrase'}});
 fireEvent.click(button);await screen.findByRole('alert');
 expect(screen.getByLabelText('New backup passphrase')).toHaveValue('');expect(screen.getByLabelText('Confirm new backup passphrase')).toHaveValue('');
});
it('requires acknowledgement and invalidates it when target changes', async () => {
 render(<I18nProvider><EncryptedBackupPanel files={['test.tar.gz.age','legacy.enc']} refresh={jest.fn()} /></I18nProvider>);
 fireEvent.change(screen.getByLabelText('Encrypted backup file'),{target:{value:'test.tar.gz.age'}});
 fireEvent.change(screen.getByLabelText('Restore directory that does not exist yet'),{target:{value:'/tmp/new'}});
 fireEvent.change(screen.getByLabelText('Passphrase for this backup'),{target:{value:'fixture passphrase'}});
 const button=screen.getByRole('button',{name:'Verify and restore to new directory'});expect(button).toBeDisabled();
 fireEvent.click(screen.getByLabelText('I will review the restored contents before applying them'));expect(button).toBeEnabled();
 fireEvent.change(screen.getByLabelText('Restore directory that does not exist yet'),{target:{value:'/tmp/changed'}});
 await waitFor(()=>expect(button).toBeDisabled());expect(screen.queryByRole('option',{name:'legacy.enc'})).not.toBeInTheDocument();
});

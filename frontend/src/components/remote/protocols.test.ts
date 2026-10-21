import { RemoteServer } from './remoteTypes';
import { addressOf, hasShell, hasTerminal, matchesProtocol, protocolsOf, sessionLabel, usesSsh } from './protocols';

const base: RemoteServer = {
  id: 'srv', name: 'web', host: 'h', port: 22, user: 'u', auth_method: 'key',
  tags: [], memo: '', created_at: '', last_checked: '', status: '',
};

test('a server saved before protocols is SSH with SFTP', () => {
  expect(protocolsOf(base)).toEqual(['ssh', 'sftp']);
  expect(usesSsh(base) && hasShell(base) && hasTerminal(base)).toBe(true);
  expect(sessionLabel(base)).toBe('ssh:u@h:22');
});

test('an FTP-only server is addressed by its FTP port and has no terminal', () => {
  const ftp = { ...base, terminal: 'none' as const, files: 'ftp' as const, ftp_port: 2121 };
  expect(protocolsOf(ftp)).toEqual(['ftp']);
  expect(addressOf(ftp)).toBe('u@h:2121');
  expect(hasTerminal(ftp) || usesSsh(ftp)).toBe(false);
  expect(matchesProtocol(ftp, 'ftp')).toBe(true);
  expect(matchesProtocol({ ...ftp, files: 'ftps' }, 'ftp')).toBe(true);
  expect(matchesProtocol(ftp, 'ssh')).toBe(false);
});

test('Telnet sessions carry their own label; SFTP-only still needs SSH approval', () => {
  const telnet = { ...base, terminal: 'telnet' as const, files: 'none' as const, port: 23 };
  expect(sessionLabel(telnet)).toBe('telnet:u@h:23');
  expect(usesSsh(telnet)).toBe(false);
  const sftpOnly = { ...base, terminal: 'none' as const };
  expect(usesSsh(sftpOnly) && !hasShell(sftpOnly)).toBe(true);
  expect(matchesProtocol(sftpOnly, 'sftp')).toBe(true);
});

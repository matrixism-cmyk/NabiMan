import React from 'react';
import { render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { RemoteServer } from './remoteTypes';
import WorkspaceTabs, { viewFor } from './WorkspaceTabs';

const base: RemoteServer = {
  id: 'srv', name: 'web', host: 'h', port: 22, user: 'u', auth_method: 'key',
  tags: [], memo: '', created_at: '', last_checked: '', status: '',
};
const ftpOnly = { ...base, terminal: 'none' as const, files: 'ftp' as const };
const telnet = { ...base, terminal: 'telnet' as const, files: 'none' as const };

beforeEach(() => localStorage.setItem('nabiman_lang', 'ko'));

const names = (server: RemoteServer) => {
  render(<I18nProvider><WorkspaceTabs selection={{ kind: 'server', id: server.id }} server={server}
    overviewView="summary" onOverviewView={jest.fn()} serverView="status" onServerView={jest.fn()}
    allSessions={0} liveCount={0} /></I18nProvider>);
  return screen.getAllByRole('button').map(b => b.textContent?.trim());
};

test('an SSH server keeps every view', () => {
  expect(names(base)).toEqual(['세션', '터미널', '상태', '실행', '환경설정']);
});

test('a files-only server shows its files instead of sessions and a terminal', () => {
  expect(names(ftpOnly)).toEqual(['파일', '상태', '환경설정']);
});

test('a Telnet server has a terminal but no commands', () => {
  expect(names(telnet)).toEqual(['세션', '터미널', '상태', '환경설정']);
});

test('a view a server does not offer falls back to one it does', () => {
  expect(viewFor(ftpOnly, 'sessions')).toBe('files');
  expect(viewFor(ftpOnly, 'terminal')).toBe('files');
  expect(viewFor(ftpOnly, 'exec')).toBe('status');
  expect(viewFor(telnet, 'exec')).toBe('status');
  expect(viewFor(telnet, 'files')).toBe('terminal');
  expect(viewFor(base, 'files')).toBe('terminal');
  expect(viewFor(base, 'exec')).toBe('exec');
  expect(viewFor(undefined, 'sessions')).toBe('sessions');
});

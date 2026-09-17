import React from 'react';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { TerminalSettingsProvider } from '../terminal/settings';
import { TerminalWindowsProvider } from '../terminal/TerminalWindows';
import RemoteWorkspace from './RemoteWorkspace';

const SERVERS = [
  { id: 'srv_1', name: 'web-prod-01', host: '192.168.1.100', port: 22, user: 'root',
    auth_method: 'password', tags: ['web'], memo: '', created_at: '', last_checked: '',
    status: 'online', has_password: true },
  { id: 'srv_2', name: 'db-replica-02', host: '192.168.1.104', port: 22, user: 'root',
    auth_method: 'key', tags: [], memo: '', created_at: '', last_checked: '',
    status: 'offline', has_password: false },
];

const SESSIONS = [
  { id: 'pty_a', label: 'local', owner: 'admin', ws_count: 1, shared: false,
    age_secs: 120, timeout_secs: 0, scrollback: 5000, keepalive: true, alive: true },
];

// The pane itself is covered by the terminal tests; here the wiring is the subject.
jest.mock('../terminal/TerminalView', () => {
  const react = require('react');
  return {
    __esModule: true,
    default: react.forwardRef(function FakeTerminalView(props: { storageKey: string }) {
      return react.createElement('div', { 'data-testid': 'terminal', 'data-key': props.storageKey });
    }),
    forgetTerminalStorage: jest.fn(),
    pruneTerminalStorage: jest.fn(),
    storedSessionId: () => '',
  };
});

jest.mock('../../hooks/useApi', () => ({
  useApi: (endpoint: string) => ({
    data: endpoint.includes('/api/terminal/sessions') ? SESSIONS
      : endpoint.includes('/api/remote-servers') && !endpoint.includes('ssh-key') ? SERVERS
      : null,
    loading: false,
    error: null,
    refetch: () => {},
  }),
  apiPost: () => Promise.resolve({ success: true, data: null, message: '' }),
  apiRequest: () => Promise.resolve({ success: true, data: null, message: '' }),
  apiDelete: () => Promise.resolve({ success: true, data: null, message: '' }),
}));

function renderWorkspace() {
  return render(
    <I18nProvider>
      <TerminalSettingsProvider>
        <TerminalWindowsProvider>
          <RemoteWorkspace />
        </TerminalWindowsProvider>
      </TerminalSettingsProvider>
    </I18nProvider>,
  );
}

const rail = () => document.querySelector('.rw-rail') as HTMLElement;
const itemByName = (name: string) =>
  Array.from(rail().querySelectorAll('.rw-item')).find(el => el.textContent?.includes(name)) as HTMLElement;

beforeEach(() => {
  localStorage.clear();
  // The operator's UI is Korean; assert against what they actually see.
  localStorage.setItem('nabiman_lang', 'ko');
  window.confirm = jest.fn(() => true);
});

describe('remote workspace', () => {
  it('lists every connection target in one rail', () => {
    renderWorkspace();
    expect(itemByName('web-prod-01')).toBeTruthy();
    expect(itemByName('db-replica-02')).toBeTruthy();
    // Local shell and the fleet overview are targets in the same list.
    expect(rail().textContent).toContain('로컬 셸');
    expect(rail().textContent).toContain('전체 현황');
  });

  it('shows how many sessions are running on a target', () => {
    renderWorkspace();
    const local = itemByName('로컬 셸');
    expect(within(local).getByText('1')).toBeInTheDocument();
  });

  it('does not dial a server just because it was selected', () => {
    renderWorkspace();
    fireEvent.click(itemByName('web-prod-01'));
    expect(screen.queryByTestId('terminal')).toBeNull();
    expect(document.querySelector('.rw-connect')).toBeTruthy();

    fireEvent.click(screen.getByText('접속'));
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_srv_srv_1');
  });

  it('attaches straight away when a session is already running', () => {
    renderWorkspace();
    fireEvent.click(itemByName('로컬 셸'));
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_local');
  });

  it('keeps the shell alive while other views of the same server are open', () => {
    renderWorkspace();
    fireEvent.click(itemByName('web-prod-01'));
    fireEvent.click(screen.getByText('접속'));

    fireEvent.click(screen.getByRole('button', { name: '상태' }));
    const pane = screen.getByTestId('terminal').closest('.rw-view') as HTMLElement;
    expect(pane.className).toContain('rw-view-hidden');
    expect(screen.getByTestId('terminal')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '터미널' }));
    expect((screen.getByTestId('terminal').closest('.rw-view') as HTMLElement).className)
      .not.toContain('rw-view-hidden');
  });

  it('offers manual password entry only where a password is stored', () => {
    renderWorkspace();
    fireEvent.click(itemByName('web-prod-01'));
    expect(screen.getByText(/직접 입력/)).toBeInTheDocument();

    fireEvent.click(itemByName('db-replica-02'));
    expect(screen.queryByText(/직접 입력/)).toBeNull();
  });

  it('reaches sessions and the SSH key from the overview', () => {
    renderWorkspace();
    fireEvent.click(itemByName('전체 현황'));
    const tabs = within(document.querySelector('.rw-tabs') as HTMLElement);
    fireEvent.click(tabs.getByRole('button', { name: /세션/ }));
    expect(screen.getByText('local')).toBeInTheDocument();
    expect(tabs.getByRole('button', { name: 'SSH 키' })).toBeInTheDocument();
  });

  it('remembers the selected target across a reload', () => {
    const first = renderWorkspace();
    fireEvent.click(itemByName('db-replica-02'));
    first.unmount();
    renderWorkspace();
    expect(document.querySelector('.rw-head-title')?.textContent).toContain('db-replica-02');
  });
});

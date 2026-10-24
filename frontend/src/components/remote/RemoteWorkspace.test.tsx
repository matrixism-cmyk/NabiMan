import React from 'react';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { TerminalSettingsProvider } from '../terminal/settings';
import { TerminalWindowsProvider } from '../terminal/TerminalWindows';
import RemoteWorkspace from './RemoteWorkspace';
import { DEFAULT_GRANTS, GrantsContext, MemberContext, MemberGrants } from './member';

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
    default: react.forwardRef(function FakeTerminalView(props: { storageKey: string; onSessionId?: (id: string) => void }) {
      // Like the real pane, it reports its session once attached.
      const { onSessionId } = props;
      react.useEffect(() => { onSessionId?.('pty_a'); }, [onSessionId]);
      return react.createElement('div', { 'data-testid': 'terminal', 'data-key': props.storageKey });
    }),
    forgetTerminalStorage: jest.fn(),
    pruneTerminalStorage: jest.fn(),
    storedSessionId: () => '',
  };
});

const mockApiRequest = jest.fn((..._args: unknown[]) =>
  Promise.resolve({ success: true, data: null, message: '' }));
const mockApiPost = jest.fn((..._args: unknown[]) =>
  Promise.resolve({ success: true, data: null, message: '' }));

jest.mock('../../shared/api/useApi', () => ({
  useApi: (endpoint: string) => ({
    data: endpoint.includes('/api/terminal/sessions') ? SESSIONS
      : endpoint.includes('/api/remote-servers') && !endpoint.includes('ssh-key') ? SERVERS
      : null,
    loading: false,
    error: null,
    refetch: () => {},
  }),
  apiPost: (...args: unknown[]) => mockApiPost(...args),
  apiRequest: (...args: unknown[]) => mockApiRequest(...args),
  apiDelete: () => Promise.resolve({ success: true, data: null, message: '' }),
}));

function renderWorkspace(member = false, grants: MemberGrants | null = member ? DEFAULT_GRANTS : null) {
  return render(
    <I18nProvider>
      <TerminalSettingsProvider>
        <TerminalWindowsProvider>
          <MemberContext.Provider value={member}>
            <GrantsContext.Provider value={grants}><RemoteWorkspace /></GrantsContext.Provider>
          </MemberContext.Provider>
        </TerminalWindowsProvider>
      </TerminalSettingsProvider>
    </I18nProvider>,
  );
}

const rail = () => document.querySelector('.rw-rail') as HTMLElement;
const tabs = () => within(document.querySelector('.rw-tabs') as HTMLElement);
const openMenuOn = (name: string) => {
  fireEvent.contextMenu(itemByName(name), { clientX: 40, clientY: 120 });
  return within(screen.getByRole('menu'));
};
const itemByName = (name: string) =>
  Array.from(rail().querySelectorAll('.rw-item')).find(el => el.textContent?.includes(name)) as HTMLElement;

beforeEach(() => {
  localStorage.clear();
  // Each test is a fresh browser tab: nothing left open by the one before.
  sessionStorage.clear();
  // The operator's UI is Korean; assert against what they actually see.
  localStorage.setItem('nabiman_lang', 'ko');
  window.confirm = jest.fn(() => true);
  // jest.config resets mock implementations between tests.
  mockApiRequest.mockImplementation(() => Promise.resolve({ success: true, data: null, message: '' }));
  mockApiPost.mockImplementation(() => Promise.resolve({ success: true, data: null, message: '' }));
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

    fireEvent.click(tabs().getByRole('button', { name: '터미널' }));
    expect(document.querySelector('.rw-connect')).toBeTruthy();
    fireEvent.click(screen.getByText('접속'));
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_srv_srv_1');
  });

  // A click on a target shows what is already running there, and does not
  // quietly attach to it either — that would count as one more viewer.
  it('opens a target on the sessions running there', () => {
    renderWorkspace();
    fireEvent.click(itemByName('로컬 셸'));
    expect(screen.queryByTestId('terminal')).toBeNull();
    expect(screen.getByText('로컬 셸 세션')).toBeInTheDocument();
    expect(document.querySelectorAll('.session-card')).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: '참여' }));
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_join_pty_a');
  });

  // The pane stays connected behind the list. Coming back to the target from
  // elsewhere must not hide the way into that session until a second click.
  it('marks the session open behind the list, every time, with a way back to it', () => {
    renderWorkspace();
    fireEvent.click(itemByName('로컬 셸'));
    fireEvent.click(tabs().getByRole('button', { name: '터미널' }));
    fireEvent.click(screen.getByText('접속'));
    fireEvent.click(itemByName('전체 현황'));

    for (let visit = 0; visit < 2; visit++) {
      fireEvent.click(itemByName('로컬 셸'));
      const card = document.querySelector('.session-card') as HTMLElement;
      expect(card.className).toContain('is-current');
      expect(within(card).queryByRole('button', { name: '참여' })).toBeNull();
      expect(within(card).getByRole('button', { name: '터미널 보기' })).toBeEnabled();
    }
    fireEvent.click(screen.getByRole('button', { name: '터미널 보기' }));
    expect((screen.getByTestId('terminal').closest('.rw-view') as HTMLElement).className).not.toContain('rw-view-hidden');
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_local');
  });

  it('says so when nothing is running on a target', () => {
    renderWorkspace();
    fireEvent.click(itemByName('web-prod-01'));
    expect(screen.getByText('web-prod-01 세션')).toBeInTheDocument();
    expect(screen.getByText(/실행 중인 세션이 없습니다/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /새 세션/ }));
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_srv_srv_1');
  });

  it('keeps the shell alive while other views of the same server are open', () => {
    renderWorkspace();
    fireEvent.click(itemByName('web-prod-01'));
    fireEvent.click(tabs().getByRole('button', { name: '터미널' }));
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
    fireEvent.click(tabs().getByRole('button', { name: '터미널' }));
    expect(screen.getByText(/직접 입력/)).toBeInTheDocument();

    fireEvent.click(itemByName('db-replica-02'));
    fireEvent.click(tabs().getByRole('button', { name: '터미널' }));
    expect(screen.queryByText(/직접 입력/)).toBeNull();
  });

  it('reaches sessions and the SSH key from the overview', () => {
    renderWorkspace();
    fireEvent.click(itemByName('전체 현황'));
    fireEvent.click(tabs().getByRole('button', { name: /^세션( \d+)?$/ }));
    // A session card shows its name and its address, so "local" appears twice.
    expect(screen.getAllByText('local').length).toBeGreaterThan(0);
    expect(tabs().getByRole('button', { name: 'SSH 키' })).toBeInTheDocument();
  });

  describe('right-click menu', () => {
    it('offers connect, edit and delete for a server', () => {
      renderWorkspace();
      const menu = openMenuOn('web-prod-01');
      for (const label of ['연결', '새 세션', '세션 목록', '새 창으로 열기', '상태', '원격 실행', '연결 확인', '수정', '삭제']) {
        expect(menu.getByRole('menuitem', { name: label })).toBeInTheDocument();
      }
    });

    it('keeps server-only actions off the local shell', () => {
      renderWorkspace();
      const menu = openMenuOn('로컬 셸');
      expect(menu.getByRole('menuitem', { name: '연결' })).toBeInTheDocument();
      expect(menu.getByRole('menuitem', { name: '세션 목록 (1)' })).toBeInTheDocument();
      expect(menu.queryByRole('menuitem', { name: '삭제' })).toBeNull();
      expect(menu.queryByRole('menuitem', { name: '수정' })).toBeNull();
    });

    it('connects from the menu', () => {
      renderWorkspace();
      fireEvent.click(openMenuOn('web-prod-01').getByRole('menuitem', { name: '연결' }));
      expect(screen.queryByRole('menu')).toBeNull();
      expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_srv_srv_1');
    });

    it('opens the edit form from the menu', () => {
      renderWorkspace();
      fireEvent.click(openMenuOn('db-replica-02').getByRole('menuitem', { name: '수정' }));
      expect(document.querySelector('.rw-head-title')?.textContent).toContain('db-replica-02');
      expect(screen.getByText('서버 정보 수정')).toBeInTheDocument();
    });

    it('deletes a server only after confirming', async () => {
      renderWorkspace();
      const deleteCall = ['/api/remote-servers/srv_2', { method: 'DELETE' }];
      (window.confirm as jest.Mock).mockReturnValueOnce(false);
      fireEvent.click(openMenuOn('db-replica-02').getByRole('menuitem', { name: '삭제' }));
      await Promise.resolve();
      expect(mockApiRequest).not.toHaveBeenCalledWith(...deleteCall);

      fireEvent.click(openMenuOn('db-replica-02').getByRole('menuitem', { name: '삭제' }));
      await Promise.resolve();
      expect(window.confirm).toHaveBeenCalledTimes(2);
      expect(mockApiRequest).toHaveBeenCalledWith(...deleteCall);
    });

    it('closes on Escape and hands focus back to the entry', () => {
      renderWorkspace();
      openMenuOn('web-prod-01');
      fireEvent.keyDown(document, { key: 'Escape' });
      expect(screen.queryByRole('menu')).toBeNull();
      expect(document.activeElement).toBe(itemByName('web-prod-01'));
    });

    it('closes on a click elsewhere', () => {
      renderWorkspace();
      openMenuOn('web-prod-01');
      fireEvent.mouseDown(document.body);
      expect(screen.queryByRole('menu')).toBeNull();
    });
  });

  it('puts an open terminal back after a reload, but not in a new tab', () => {
    const first = renderWorkspace();
    fireEvent.click(itemByName('web-prod-01'));
    fireEvent.click(tabs().getByRole('button', { name: '터미널' }));
    fireEvent.click(screen.getByText('접속'));
    first.unmount();

    const second = renderWorkspace();
    expect(screen.getByTestId('terminal')).toHaveAttribute('data-key', 'ws_srv_srv_1');
    second.unmount();

    sessionStorage.clear();
    renderWorkspace();
    expect(screen.queryByTestId('terminal')).toBeNull();
    expect(screen.getByText('web-prod-01 세션')).toBeInTheDocument();
  });

  it('remembers the selected target across a reload', () => {
    const first = renderWorkspace();
    fireEvent.click(itemByName('db-replica-02'));
    first.unmount();
    renderWorkspace();
    expect(document.querySelector('.rw-head-title')?.textContent).toContain('db-replica-02');
  });

  // A remote-only member: their servers, and nothing of this machine's or
  // the administrators' — not even offered.
  it('offers a member only their own servers', () => {
    localStorage.setItem('nabiman_remote_selection', JSON.stringify({ kind: 'local' }));
    renderWorkspace(true);
    expect(itemByName('web-prod-01')).toBeTruthy();
    expect(rail().textContent).not.toContain('로컬 셸');
    expect(rail().textContent).not.toContain('임시 접속');
    expect(rail().textContent).not.toContain('전체 확인');
    // A target of this machine's remembered from before falls back to the overview.
    expect(screen.queryByText('로컬 셸 세션')).toBeNull();
    expect(tabs().getByRole('button', { name: '요약' })).toBeInTheDocument();
    expect(tabs().queryByRole('button', { name: 'SSH 키' })).toBeNull();
    expect(tabs().queryByRole('button', { name: '공유 링크' })).toBeNull();
    expect(screen.queryByRole('button', { name: '전체 확인' })).toBeNull();

    fireEvent.click(itemByName('web-prod-01'));
    expect(tabs().queryByRole('button', { name: '실행' })).toBeNull();
    // An administrators' server assigned to them: worked on, never edited.
    expect(within(itemByName('web-prod-01')).getByText('관리자 지정')).toBeInTheDocument();
    expect(tabs().queryByRole('button', { name: '환경설정' })).toBeNull();
    const menu = openMenuOn('web-prod-01');
    expect(menu.queryByText('원격 실행')).toBeNull();
    expect(menu.queryByRole('menuitem', { name: '삭제' })).toBeNull();
    expect(menu.getByRole('menuitem', { name: '연결' })).toBeInTheDocument();
  });

  // 세션 공유 granted by an administrator: the member's sessions get the link.
  it('offers a member session links only once granted', () => {
    renderWorkspace(true, { ...DEFAULT_GRANTS, share_sessions: true });
    expect(tabs().getByRole('button', { name: '공유 링크' })).toBeInTheDocument();
    fireEvent.click(tabs().getByRole('button', { name: /^세션( \d+)?$/ }));
    const links = () => Array.from(document.querySelectorAll('.session-actions button')).filter(b => b.textContent === '🔗');
    expect(links()).toHaveLength(1);
    expect(screen.queryByText('내부 사용자 공유')).toBeNull();
  });
});

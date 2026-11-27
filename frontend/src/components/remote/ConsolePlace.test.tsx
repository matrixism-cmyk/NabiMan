import React from 'react';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { TerminalSettingsProvider } from '../terminal/settings';
import { TerminalWindowsProvider } from '../terminal/TerminalWindows';
import RemoteWorkspace from './RemoteWorkspace';
import { consoleKeeper } from './consoleKeeper';

const CONSOLES = [
  { name: 'win11', label: 'Windows 11', builtin: true, protocol: 'rdp' },
  { name: 'rocky8', label: 'Rocky Linux 8', builtin: true, protocol: 'vnc' },
];

jest.mock('../terminal/TerminalView', () => {
  const react = require('react');
  return { __esModule: true, default: react.forwardRef(() => react.createElement('div', { 'data-testid': 'terminal' })) };
});
/** How many views hold each console's screen right now, and the most there ever were at once. */
const mockHeld: Record<string, number> = {};
let mockPeak = 0;
// Stand-ins for the console views: where each is shown, and the two ways out of the pane.
jest.mock('./lazyConsoles', () => {
  const react = require('react');
  const fake = (props: Record<string, any>) => {
    const name = props.target || props.vm;
    // As the real views: the screen is taken in an effect and given back in its cleanup.
    react.useEffect(() => {
      mockHeld[name] = (mockHeld[name] || 0) + 1;
      mockPeak = Math.max(mockPeak, mockHeld[name]);
      return () => { mockHeld[name] -= 1; };
    }, [name]);
    return react.createElement('div', { 'data-testid': `console-${name}`, 'data-place': props.windowed ? 'window' : 'pane' },
      props.onDetach && react.createElement('button', { onClick: props.onDetach }, 'view-detach'),
      props.onPopOut && react.createElement('button', { onClick: props.onPopOut }, 'view-popout'));
  };
  return { RdpConsoleView: fake, VncConsoleView: fake, ConsoleSuspense: ({ children }: { children: unknown }) => children };
});
jest.mock('./ConsoleShareModal', () => () => null);
jest.mock('./ConsoleAccessModal', () => () => null);
jest.mock('../../shared/api/useApi', () => ({
  sessionSubject: () => null,
  useApi: (endpoint: string) => ({
    data: endpoint.includes('/api/remote/consoles') ? CONSOLES : endpoint.includes('/api/users/me') ? { role: 'admin' } : endpoint.includes('ssh-key') ? null : [],
    loading: false, error: null, refetch: () => {},
  }),
  apiPost: () => Promise.resolve({ success: true, data: null, message: '' }),
  apiRequest: (path: string) => Promise.resolve({ success: true, data: path === '/api/remote/consoles' ? CONSOLES : null, message: '' }),
  apiDelete: () => Promise.resolve({ success: true, data: null, message: '' }),
}));

const workspace = () => render(
  <I18nProvider><TerminalSettingsProvider enabled={false}><TerminalWindowsProvider owner="kim"><RemoteWorkspace /></TerminalWindowsProvider></TerminalSettingsProvider></I18nProvider>,
);
const rail = () => document.querySelector('.rw-rail') as HTMLElement;
const railItem = (name: string) => Array.from(rail().querySelectorAll('.rw-item')).find((el) => el.textContent?.includes(name)) as HTMLElement;
const menuOn = (name: string) => { fireEvent.contextMenu(railItem(name), { clientX: 40, clientY: 120 }); return within(screen.getByRole('menu')); };
const pane = () => document.querySelector('.rw-body') as HTMLElement;
const consoleFrames = () => Array.from(document.querySelectorAll<HTMLElement>('.term-window[data-window-kind="console"]'));
const place = (name: string) => screen.getByTestId(`console-${name}`).getAttribute('data-place');

beforeEach(() => {
  localStorage.clear(); sessionStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
  Object.keys(mockHeld).forEach((k) => delete mockHeld[k]);
  mockPeak = 0;
});
afterEach(() => { consoleKeeper.dropAll(); jest.restoreAllMocks(); });

test('the pane\'s "분리" moves the console to a window on the page; the pane lets go and shows the overview', () => {
  const opened = jest.spyOn(window, 'open');
  workspace();
  fireEvent.click(railItem('Windows 11'));
  expect(place('win11')).toBe('pane');
  fireEvent.click(screen.getByText('view-detach'));
  expect(place('win11')).toBe('window');
  expect(screen.getAllByTestId('console-win11')).toHaveLength(1);
  expect(consoleFrames()).toHaveLength(1);
  expect(railItem('Windows 11')).not.toHaveClass('is-active');
  expect(pane().querySelector('.rw-console-elsewhere')).toBeNull();
  // Not a browser window, and never two views on the one screen.
  expect(opened).not.toHaveBeenCalled();
  expect(mockPeak).toBe(1);
});

test('the rail\'s right-click offers both: "분리" opens the window on the page, "새창으로 분리" the browser window', () => {
  const opened = jest.spyOn(window, 'open').mockReturnValue({ focus: jest.fn() } as unknown as Window);
  workspace();
  const menu = menuOn('Rocky Linux 8');
  expect(menu.getByRole('menuitem', { name: /^⧉?\s*분리$/ }).title).toContain('아래 줄');
  expect(menu.getByRole('menuitem', { name: /새창으로 분리/ }).title).toContain('다른 모니터');
  expect(menu.getByRole('menuitem', { name: /새 연결/ })).toBeInTheDocument();
  fireEvent.click(menu.getByRole('menuitem', { name: /^⧉?\s*분리$/ }));
  expect(consoleFrames()).toHaveLength(1);
  expect(place('rocky8')).toBe('window');
  expect(opened).not.toHaveBeenCalled();

  // Out to a browser window: this tab hangs up, and the window here goes.
  const close = jest.fn();
  act(() => consoleKeeper.adopt('rocky8', { close, alive: () => true, watch: () => () => undefined }));
  fireEvent.click(menuOn('Rocky Linux 8').getByRole('menuitem', { name: /새창으로 분리/ }));
  expect(close).toHaveBeenCalled();
  expect(opened).toHaveBeenCalledWith('/remote#console/rocky8', 'con_rocky8', expect.stringContaining('width=1300'));
  expect(consoleFrames()).toHaveLength(0);
  expect(screen.queryByTestId('console-rocky8')).toBeNull();
});

test('"새 연결" on the rail stays a browser window of its own and touches nothing here', () => {
  const opened = jest.spyOn(window, 'open').mockReturnValue({ focus: jest.fn() } as unknown as Window);
  workspace();
  fireEvent.click(railItem('Windows 11'));
  fireEvent.click(menuOn('Windows 11').getByRole('menuitem', { name: /새 연결/ }));
  expect(opened).toHaveBeenCalledWith('/remote#rdp/win11', expect.stringMatching(/^con_win11_\d+$/), expect.any(String));
  expect(place('win11')).toBe('pane');
  expect(consoleFrames()).toHaveLength(0);
});

test('the pane\'s "새창으로 분리" hands the console to a browser window and the pane lets go', () => {
  const opened = jest.spyOn(window, 'open').mockReturnValue({ focus: jest.fn() } as unknown as Window);
  workspace();
  fireEvent.click(railItem('Windows 11'));
  fireEvent.click(screen.getByText('view-popout'));
  expect(opened).toHaveBeenCalledWith('/remote#rdp/win11', 'con_win11', expect.any(String));
  expect(screen.queryByTestId('console-win11')).toBeNull();
  expect(railItem('Windows 11')).not.toHaveClass('is-active');
  expect(consoleFrames()).toHaveLength(0);
});

test('a window opened from the rail for the console the pane shows takes it over (pane → window)', () => {
  workspace();
  fireEvent.click(railItem('Rocky Linux 8'));
  expect(place('rocky8')).toBe('pane');
  fireEvent.click(menuOn('Rocky Linux 8').getByRole('menuitem', { name: /^⧉?\s*분리$/ }));
  expect(screen.getAllByTestId('console-rocky8')).toHaveLength(1);
  expect(place('rocky8')).toBe('window');
  expect(pane().querySelector('.rw-console-elsewhere')).toBeNull();
  expect(mockPeak).toBe(1);
});

test('selecting in the rail a console that is in a window does not steal it: a notice with both ways on', () => {
  workspace();
  fireEvent.click(menuOn('Rocky Linux 8').getByRole('menuitem', { name: /^⧉?\s*분리$/ }));
  fireEvent.click(menuOn('Windows 11').getByRole('menuitem', { name: /^⧉?\s*분리$/ }));
  const z = () => consoleFrames().map((f) => Number(f.style.zIndex));
  expect(z()[1]).toBeGreaterThan(z()[0]);

  fireEvent.click(railItem('Rocky Linux 8'));
  expect(within(pane()).getByText('이 콘솔은 창으로 열려 있습니다')).toBeInTheDocument();
  expect(place('rocky8')).toBe('window');
  expect(screen.getAllByTestId('console-rocky8')).toHaveLength(1);

  // "창으로 가기": the window comes to the front — and back on the page when it was minimised.
  const frame = consoleFrames()[0];
  fireEvent.click(frame.querySelector('[title="최소화"]') as HTMLElement);
  expect(frame).toHaveClass('term-window-minimized');
  fireEvent.click(within(pane()).getByRole('button', { name: '창으로 가기' }));
  expect(frame).not.toHaveClass('term-window-minimized');
  expect(z()[0]).toBeGreaterThan(z()[1]);
  expect(within(pane()).getByText('이 콘솔은 창으로 열려 있습니다')).toBeInTheDocument();

  // "여기로 가져오기": the window closes and the pane shows the console.
  fireEvent.click(within(pane()).getByRole('button', { name: '여기로 가져오기' }));
  expect(consoleFrames()).toHaveLength(1);
  expect(place('rocky8')).toBe('pane');
  expect(screen.getAllByTestId('console-rocky8')).toHaveLength(1);
  expect(place('win11')).toBe('window');
  expect(mockPeak).toBe(1);
});

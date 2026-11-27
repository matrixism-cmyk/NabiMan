import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { consoleKeeper } from '../remote/consoleKeeper';
import { TerminalSettingsProvider } from './settings';
import { TerminalWindowsProvider, useTerminalWindows } from './TerminalWindows';

jest.mock('./TerminalView', () => {
  const react = require('react');
  return { __esModule: true, default: react.forwardRef(() => react.createElement('div', { 'data-testid': 'fake-terminal' })) };
});
// The window manager is under test: the console views are stand-ins that show what they were given.
jest.mock('../remote/lazyConsoles', () => {
  const react = require('react');
  const fake = (protocol: string) => (props: Record<string, unknown>) => react.createElement('div', {
    'data-testid': `fake-${protocol}`, 'data-console': props.target || props.vm, 'data-windowed': String(props.windowed), 'data-focused': String(props.focused),
  });
  return { RdpConsoleView: fake('rdp'), VncConsoleView: fake('vnc'), ConsoleSuspense: ({ children }: { children: unknown }) => children };
});
jest.mock('../remote/ConsoleShareModal', () => () => null);
let mockConsoles: (() => Promise<unknown>) = () => Promise.resolve({ success: true, data: [] });
jest.mock('../../shared/api/useApi', () => ({
  sessionSubject: () => null,
  apiRequest: (path: string) => (path === '/api/remote/consoles' ? mockConsoles() : Promise.resolve({ success: true, message: '', data: null })),
}));

const rocky = { name: 'rocky8', label: 'Rocky Linux 8', builtin: true, protocol: 'vnc' as const };
const win11 = { name: 'win11', label: 'Windows 11', builtin: true, protocol: 'rdp' as const };

function Opener() {
  const { openTerminal, openConsole } = useTerminalWindows();
  return (
    <>
      <button onClick={() => openTerminal({ kind: 'local' }, { title: 'shell' })}>term</button>
      <button onClick={() => openConsole(rocky)}>rocky</button>
      <button onClick={() => openConsole(win11)}>win11</button>
    </>
  );
}
const desktop = () => render(
  <I18nProvider><TerminalSettingsProvider><TerminalWindowsProvider owner="kim"><Opener /></TerminalWindowsProvider></TerminalSettingsProvider></I18nProvider>,
);
const frames = () => Array.from(document.querySelectorAll<HTMLElement>('.term-window'));
const consoleFrames = () => frames().filter((f) => f.dataset.windowKind === 'console');
const barItems = () => Array.from(document.querySelectorAll<HTMLElement>('.term-taskbar-item'));
const press = (el: Element) => { fireEvent.pointerDown(el, { bubbles: true, button: 0 }); fireEvent.click(el); };

beforeEach(() => {
  localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
  mockConsoles = () => Promise.resolve({ success: true, data: [rocky, win11] });
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
  Object.defineProperty(window, 'innerHeight', { value: 1000, configurable: true });
});
afterEach(() => { consoleKeeper.dropAll(); jest.restoreAllMocks(); });

test('a console opens as a window in the same layer, with its view compact inside and a marked item on the bar', () => {
  desktop();
  fireEvent.click(screen.getByText('term'));
  fireEvent.click(screen.getByText('rocky'));
  fireEvent.click(screen.getByText('win11'));
  expect(frames()).toHaveLength(3);
  expect(document.querySelector('.term-window-layer')?.querySelectorAll('.term-window')).toHaveLength(3);
  expect(screen.getByTestId('fake-vnc')).toHaveAttribute('data-console', 'rocky8');
  expect(screen.getByTestId('fake-vnc')).toHaveAttribute('data-windowed', 'true');
  expect(screen.getByTestId('fake-rdp')).toHaveAttribute('data-console', 'win11');
  // The bar counts every window, and tells a console by a word.
  expect(screen.getByRole('button', { name: /창 3/ })).toBeInTheDocument();
  expect(barItems().map((b) => b.textContent)).toEqual(['▪ shell', '▪ VMRocky Linux 8', '▪ VMWindows 11']);
  expect(barItems()[1].querySelector('.term-kind-tag')).toHaveAccessibleName('VM 콘솔');
  expect(barItems()[0].querySelector('.term-kind-tag')).toBeNull();
});

test('opening a console that already has a window raises that window instead of opening another', () => {
  desktop();
  fireEvent.click(screen.getByText('rocky'));
  fireEvent.click(screen.getByText('term'));
  const [consoleFrame] = consoleFrames();
  const before = Number(consoleFrame.style.zIndex);
  press(consoleFrame.querySelector('[title="최소화"]') as HTMLElement);
  expect(consoleFrame).toHaveClass('term-window-minimized');
  fireEvent.click(screen.getByText('rocky'));
  expect(consoleFrames()).toHaveLength(1);
  expect(consoleFrame).not.toHaveClass('term-window-minimized');
  expect(Number(consoleFrame.style.zIndex)).toBeGreaterThan(before);
  expect(screen.getAllByTestId('fake-vnc')).toHaveLength(1);
});

test('only the focused console window has the keys; a minimised one stays mounted without them', () => {
  desktop();
  fireEvent.click(screen.getByText('rocky'));
  fireEvent.click(screen.getByText('win11'));
  expect(screen.getByTestId('fake-vnc')).toHaveAttribute('data-focused', 'false');
  expect(screen.getByTestId('fake-rdp')).toHaveAttribute('data-focused', 'true');
  // A press anywhere in the other window brings it to the front, and the keys with it.
  fireEvent.pointerDown(screen.getByTestId('fake-vnc'), { bubbles: true, button: 0 });
  expect(screen.getByTestId('fake-vnc')).toHaveAttribute('data-focused', 'true');
  expect(screen.getByTestId('fake-rdp')).toHaveAttribute('data-focused', 'false');
  press(consoleFrames()[0].querySelector('[title="최소화"]') as HTMLElement);
  expect(screen.getByTestId('fake-vnc')).toHaveAttribute('data-focused', 'false');
  expect(screen.getByTestId('fake-vnc')).toBeInTheDocument();
  expect(screen.getByTestId('fake-rdp')).toHaveAttribute('data-focused', 'true');
});

test('tile, cascade and hide all / show all take consoles along with terminals', () => {
  desktop();
  fireEvent.click(screen.getByText('term'));
  fireEvent.click(screen.getByText('rocky'));
  fireEvent.click(screen.getByText('win11'));
  fireEvent.click(screen.getByText(/정렬/));
  expect(frames().map((f) => `${f.style.left}|${f.style.top}`)).toEqual(['3px|3px', '963px|3px', '3px|481px']);
  fireEvent.click(screen.getByText(/계단식/));
  expect(frames().map((f) => f.style.left)).toEqual(['40px', '72px', '104px']);
  fireEvent.click(screen.getByText(/모두 숨기기/));
  expect(frames().every((f) => f.classList.contains('term-window-minimized'))).toBe(true);
  expect(screen.getByTestId('fake-vnc')).toBeInTheDocument();
  fireEvent.click(screen.getByText(/모두 보이기/));
  expect(frames().some((f) => f.classList.contains('term-window-minimized'))).toBe(false);
});

test('✕ closes a console window without asking; its place and size come back after a reload', async () => {
  const first = desktop();
  fireEvent.click(screen.getByText('rocky'));
  fireEvent.click(screen.getByText('win11'));
  press(consoleFrames()[1].querySelector('.term-window-close') as HTMLElement);
  expect(consoleFrames()).toHaveLength(1);
  const { left, top, width, height } = consoleFrames()[0].style;
  expect([left, top, width, height]).toEqual(['60px', '70px', '1040px', '720px']);
  first.unmount();

  desktop();
  // Until the account's console list has loaded the window waits: it neither connects nor goes.
  expect(consoleFrames()).toHaveLength(1);
  expect(screen.queryByTestId('fake-vnc')).toBeNull();
  expect(await screen.findByTestId('fake-vnc')).toHaveAttribute('data-console', 'rocky8');
  expect(consoleFrames()[0].style).toMatchObject({ left, top, width, height });
});

test('a saved console window goes once the list has loaded without its console — not before', async () => {
  const first = desktop();
  fireEvent.click(screen.getByText('term'));
  fireEvent.click(screen.getByText('rocky'));
  fireEvent.click(screen.getByText('win11'));
  first.unmount();

  let answer: (value: unknown) => void = () => undefined;
  mockConsoles = () => new Promise((resolve) => { answer = resolve; });
  desktop();
  expect(consoleFrames()).toHaveLength(2);
  await act(async () => { answer({ success: true, data: [win11] }); });
  expect(consoleFrames()).toHaveLength(1);
  expect(screen.getByTestId('fake-rdp')).toBeInTheDocument();
  expect(frames()).toHaveLength(2);
});

test('an account that may not open consoles gets none back; an unreachable server drops nothing', async () => {
  const first = desktop();
  fireEvent.click(screen.getByText('term'));
  fireEvent.click(screen.getByText('rocky'));
  first.unmount();

  mockConsoles = () => Promise.reject(new Error('offline'));
  const offline = desktop();
  expect(await screen.findByTestId('fake-vnc')).toBeInTheDocument();
  offline.unmount();

  mockConsoles = () => Promise.resolve({ success: false, message: 'forbidden', data: null });
  desktop();
  await waitFor(() => expect(consoleFrames()).toHaveLength(0));
  expect(frames()).toHaveLength(1);
  expect(screen.queryByTestId('fake-vnc')).toBeNull();
});

test('"새창으로 분리" on the title bar hangs up here, opens the browser window and closes this one', () => {
  const close = jest.fn();
  consoleKeeper.adopt('win11', { close, alive: () => true, watch: () => () => undefined });
  const focus = jest.fn();
  const opened = jest.spyOn(window, 'open').mockImplementation(() => { expect(close).toHaveBeenCalled(); return { focus } as unknown as Window; });
  desktop();
  fireEvent.click(screen.getByText('win11'));
  fireEvent.click(screen.getByText('rocky'));
  const button = consoleFrames()[0].querySelector('.term-window-popout') as HTMLElement;
  expect(button).toHaveTextContent('새창으로 분리');
  expect(button.title).toContain('다른 모니터');
  press(button);
  expect(opened).toHaveBeenCalledWith('/remote#rdp/win11', 'con_win11', expect.stringContaining('width=1300'));
  expect(consoleKeeper.get('win11')).toBeUndefined();
  expect(focus).toHaveBeenCalled();
  expect(consoleFrames()).toHaveLength(1);
  expect(screen.queryByTestId('fake-rdp')).toBeNull();
});

test('a browser that refuses the new window leaves the console window where it is', () => {
  jest.spyOn(window, 'open').mockReturnValue(null);
  desktop();
  fireEvent.click(screen.getByText('rocky'));
  press(consoleFrames()[0].querySelector('.term-window-popout') as HTMLElement);
  expect(consoleFrames()).toHaveLength(1);
});

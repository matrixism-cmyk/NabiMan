import React from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import RdpConsoleView from './RdpConsoleView';
import { consoleKeeper } from './consoleKeeper';

/** Every Guacamole client the view (or its session) made, oldest first. */
const mockClients: any[] = [];

jest.mock('../../vendor/guacamole-common.js', () => {
  class Client {
    connected: string | null = null;
    disconnected = false;
    keys: [number, number][] = [];
    element = global.document.createElement('div');
    onstatechange: ((state: number) => void) | null = null;
    onerror: ((status: { code?: number; message?: string }) => void) | null = null;
    constructor() { mockClients.push(this); }
    getDisplay() {
      return { getElement: () => this.element, getWidth: () => 1280, getHeight: () => 800, getScale: () => 1, scale: () => undefined, onresize: null };
    }
    connect(query: string) { this.connected = query; }
    disconnect() { this.disconnected = true; }
    sendKeyEvent(down: number, keysym: number) { this.keys.push([down, keysym]); }
    sendMouseState() { /* not looked at */ }
    sendSize() { /* not looked at */ }
  }
  class Pointer { }
  /** As Guacamole.Keyboard: keys of the element it was given, and `reset` lets go of the ones held. */
  class Keyboard {
    pressed: Record<number, boolean> = {};
    onkeydown: ((keysym: number) => void) | null = null;
    onkeyup: ((keysym: number) => void) | null = null;
    constructor(element: HTMLElement) {
      element.addEventListener('keydown', (e) => { this.pressed[e.keyCode] = true; this.onkeydown?.(e.keyCode); });
      element.addEventListener('keyup', (e) => { delete this.pressed[e.keyCode]; this.onkeyup?.(e.keyCode); });
    }
    reset() { Object.keys(this.pressed).forEach((k) => { delete this.pressed[Number(k)]; this.onkeyup?.(Number(k)); }); }
  }
  const Mouse: any = Pointer;
  Mouse.State = class {};
  Mouse.Touchpad = Pointer;
  Mouse.Touchscreen = Pointer;
  return { __esModule: true, default: { Client, WebSocketTunnel: class {}, Mouse, Keyboard } };
});
jest.mock('./vncApi', () => ({ rdpTicket: () => Promise.resolve('f'.repeat(64)), consoleCreds: () => Promise.resolve('creds') }));
jest.mock('./ConsoleDictation', () => () => null);
jest.mock('./RdpDrivePanel', () => () => null);

const view = (props: Partial<React.ComponentProps<typeof RdpConsoleView>> = {}) => (
  <I18nProvider><RdpConsoleView target="win11" label="Windows 11" {...props} /></I18nProvider>
);
/** Lets the ticket arrive, then guacd report the desktop is up. */
const connect = async (client = mockClients[mockClients.length - 1]) => {
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });
  act(() => client.onstatechange(3));
};
const host = () => document.querySelector('.rw-rdp-screen') as HTMLElement;

beforeEach(() => {
  localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
  mockClients.length = 0;
});
afterEach(() => { act(() => consoleKeeper.dropAll()); jest.useRealTimers(); });

test('leaving the console keeps its connection, and coming back shows the same one at once', async () => {
  const first = render(view());
  await connect();
  expect(mockClients).toHaveLength(1);
  expect(mockClients[0].connected).toContain('target=win11');
  expect(screen.getByText('연결됨')).toBeInTheDocument();
  expect(host().contains(mockClients[0].element)).toBe(true);

  first.unmount();
  expect(mockClients[0].disconnected).toBe(false);
  expect(consoleKeeper.hidden()).toEqual(['win11']);
  expect(document.body.contains(mockClients[0].element)).toBe(false);

  render(view());
  // No new connection, no "connecting" in between: the live screen is simply put back.
  expect(mockClients).toHaveLength(1);
  expect(screen.getByText('연결됨')).toBeInTheDocument();
  expect(host().contains(mockClients[0].element)).toBe(true);
  expect(consoleKeeper.hidden()).toEqual([]);
});

test('a key still held when the console is left goes up in the VM, and no key reaches it afterwards', async () => {
  const first = render(view());
  await connect();
  const el = host();
  fireEvent.keyDown(el, { keyCode: 0x61 });
  expect(mockClients[0].keys).toEqual([[1, 0x61]]);

  first.unmount();
  expect(mockClients[0].keys).toEqual([[1, 0x61], [0, 0x61]]);
  // The old screen element is off the page, and its keyboard is unhooked.
  fireEvent.keyDown(el, { keyCode: 0x62 });
  fireEvent.keyDown(document.body, { keyCode: 0x62 });
  expect(mockClients[0].keys).toHaveLength(2);
});

test('a popout window owns its connection and closes it with the window', async () => {
  const popout = render(view({ popout: true }));
  await connect();
  expect(consoleKeeper.size()).toBe(0);
  expect(screen.queryByRole('button', { name: '연결 끊기' })).toBeNull();
  popout.unmount();
  expect(mockClients[0].disconnected).toBe(true);
});

test('연결 끊기 hangs up for good; 다시 연결 connects again', async () => {
  render(view());
  await connect();
  fireEvent.click(screen.getByRole('button', { name: '연결 끊기' }));
  expect(mockClients[0].disconnected).toBe(true);
  expect(consoleKeeper.size()).toBe(0);
  expect(screen.getByText('끊김')).toBeInTheDocument();

  fireEvent.click(screen.getByRole('button', { name: '다시 연결' }));
  await connect();
  expect(mockClients).toHaveLength(2);
  expect(consoleKeeper.get('win11')?.alive()).toBe(true);
});

test('choosing a speed reconnects with it and remembers it for this console', async () => {
  const first = render(view());
  await connect();
  expect(mockClients[0].connected).toContain('speed=balanced');

  fireEvent.change(screen.getByRole('combobox', { name: '화면 속도' }), { target: { value: 'fast' } });
  await connect();
  expect(mockClients).toHaveLength(2);
  expect(mockClients[0].disconnected).toBe(true);
  expect(mockClients[1].connected).toContain('speed=fast');
  expect(JSON.parse(localStorage.getItem('nabiman_console_speed') || '{}')).toEqual({ win11: 'fast' });

  // Coming back later: still the same connection, still "fast".
  first.unmount();
  render(view());
  expect(mockClients).toHaveLength(2);
  expect((screen.getByRole('combobox', { name: '화면 속도' }) as HTMLSelectElement).value).toBe('fast');
});

test('thrown out by another login: says so and waits, instead of taking the session back', async () => {
  jest.useFakeTimers();
  render(view());
  await connect();
  act(() => {
    mockClients[0].onerror({ code: 0x0209, message: 'RDP server closed/refused connection: Disconnected by other connection.' });
    mockClients[0].onstatechange(5);
  });
  expect(screen.getByRole('alert')).toHaveTextContent('다른 곳에서 이 계정으로 접속해 연결이 끊겼습니다');
  act(() => { jest.advanceTimersByTime(60_000); });
  expect(mockClients).toHaveLength(1);

  fireEvent.click(screen.getByRole('button', { name: '다시 연결' }));
  expect(mockClients).toHaveLength(2);
});

test('an ordinary drop still reconnects by itself', async () => {
  jest.useFakeTimers();
  render(view());
  await connect();
  act(() => { mockClients[0].onstatechange(5); });
  expect(screen.getByRole('alert')).toHaveTextContent('재연결 중… (1/4)');
  act(() => { jest.advanceTimersByTime(1500); });
  expect(mockClients).toHaveLength(2);
});

import React from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import VncConsoleView from './VncConsoleView';
import { consoleKeeper } from './consoleKeeper';

/** Every noVNC connection made, oldest first. */
const mockRfbs: any[] = [];

jest.mock('@novnc/novnc/lib/rfb.js', () => {
  class RFB extends EventTarget {
    scaleViewport = false;
    resizeSession = false;
    qualityLevel = 6;
    compressionLevel = 2;
    viewOnly = false;
    background = '';
    disconnected = false;
    blurred = 0;
    target: any;
    url: string;
    constructor(target: any, url: string) { super(); this.target = target; this.url = url; mockRfbs.push(this); }
    disconnect() { this.disconnected = true; }
    blur() { this.blurred += 1; }
  }
  return { __esModule: true, default: RFB };
});
jest.mock('./vncApi', () => ({ consoleWsUrl: (vm: string) => Promise.resolve(`ws://test/console?vm=${vm}`), shareWsUrl: () => 'ws://test/share' }));
jest.mock('./ConsoleDictation', () => () => null);

const view = (props: Partial<React.ComponentProps<typeof VncConsoleView>> = {}) => (
  <I18nProvider><VncConsoleView vm="ubuntu" label="Ubuntu" {...props} /></I18nProvider>
);
const connect = async () => {
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });
  act(() => { mockRfbs[mockRfbs.length - 1].dispatchEvent(new Event('connect')); });
};

beforeEach(() => { localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko'); mockRfbs.length = 0; });
afterEach(() => { act(() => consoleKeeper.dropAll()); });

test('leaving a VNC console keeps it connected; coming back puts the same screen on the page', async () => {
  const first = render(view());
  await connect();
  const rfb = mockRfbs[0];
  expect(screen.getByText('연결됨')).toBeInTheDocument();
  expect(document.body.contains(rfb.target)).toBe(true);

  // Following the window's size (화면 맞춤 off) asks the VM to resize — only while on the page.
  fireEvent.click(screen.getByRole('checkbox'));
  expect(rfb.resizeSession).toBe(true);

  first.unmount();
  expect(rfb.disconnected).toBe(false);
  expect(rfb.blurred).toBe(1);
  expect(rfb.resizeSession).toBe(false);
  expect(document.body.contains(rfb.target)).toBe(false);
  expect(consoleKeeper.hidden()).toEqual(['ubuntu']);

  render(view());
  expect(mockRfbs).toHaveLength(1);
  expect(screen.getByText('연결됨')).toBeInTheDocument();
  expect(document.body.contains(rfb.target)).toBe(true);
});

test('the speed choice sets noVNC quality on the live connection, without reconnecting', async () => {
  render(view());
  await connect();
  expect([mockRfbs[0].qualityLevel, mockRfbs[0].compressionLevel]).toEqual([6, 2]);
  fireEvent.change(screen.getByRole('combobox', { name: '화면 속도' }), { target: { value: 'fast' } });
  expect([mockRfbs[0].qualityLevel, mockRfbs[0].compressionLevel]).toEqual([3, 6]);
  expect(mockRfbs).toHaveLength(1);
});

test('a share page owns its connection and closes it with the page', async () => {
  const page = render(view({ popout: true, share: { token: 'tok', viewOnly: true } }));
  await connect();
  expect(mockRfbs[0].viewOnly).toBe(true);
  expect(consoleKeeper.size()).toBe(0);
  page.unmount();
  expect(mockRfbs[0].disconnected).toBe(true);
});

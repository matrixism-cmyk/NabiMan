import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { BroadcastChannel as NodeChannel } from 'worker_threads';
import { TextDecoder as NodeDecoder, TextEncoder as NodeEncoder } from 'util';
import { I18nProvider } from '../../i18n';
import SerialMonitorPopout from './SerialMonitorPopout';
import { channelName, monitorTarget, monitorUrl } from './serialMonitorChannel';
import { monitorChannel } from './serialDevices';

// The screen: what was written to it, in order.
let screenText = '';
jest.mock('xterm', () => ({
  Terminal: class {
    loadAddon() { /* no addons in a test */ }
    open() { /* nothing to draw on */ }
    write(s: string) { screenText += s; }
    clear() { screenText = ''; }
    dispose() { /* done */ }
  },
}));
jest.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit() { /* fixed size */ } } }));

// jsdom has no BroadcastChannel (nor TextDecoder); Node's carry the messages between instances in one process.
(globalThis as any).BroadcastChannel = NodeChannel;
(globalThis as any).TextDecoder = NodeDecoder;
(globalThis as any).TextEncoder = NodeEncoder;

const opened: NodeChannel[] = [];
const holderFor = (sid: string, name: string) => { const c = new NodeChannel(channelName(sid, name)); opened.push(c); return c; };
afterEach(() => { opened.splice(0).forEach((c) => c.close()); });
beforeEach(() => { screenText = ''; localStorage.setItem('nabiman_lang', 'ko'); });

const view = (sid: string, name: string) => render(<I18nProvider><SerialMonitorPopout sid={sid} name={name} /></I18nProvider>);

it('the monitor page is found from its address, and only there', () => {
  expect(monitorUrl('pty_1', 'esp32')).toBe('/remote#serial/pty_1/esp32');
  expect(monitorTarget('/remote', '#serial/pty_1/esp32')).toEqual({ sid: 'pty_1', name: 'esp32' });
  expect(monitorTarget('/remote', '#console/vm')).toBeNull();
  expect(monitorTarget('/other', '#serial/pty_1/esp32')).toBeNull();
});

it('shows what the board said before and since, and sends, resets through the tab holding the port', async () => {
  const holder = holderFor('pty_1', 'esp32');
  const asked: any[] = [];
  holder.onmessage = (e: any) => {
    asked.push(e.data);
    if (e.data.type === 'hello') {
      holder.postMessage({ type: 'backlog', text: 'ESP-ROM:esp32c3\r\n' });
      holder.postMessage({ type: 'state', state: 'connected', baud: 115200, path: '/s/esp32', device: `${0x303a}:${0x1001}`, label: 'COM5' });
      holder.postMessage({ type: 'data', bytes: new TextEncoder().encode('I (12) app: 안녕\r\n') });
    }
  };
  view('pty_1', 'esp32');
  await waitFor(() => expect(screenText).toContain('I (12) app: 안녕'), { timeout: 5000 });
  expect(screenText.indexOf('ESP-ROM:esp32c3')).toBeLessThan(screenText.indexOf('여기부터 실시간'));
  expect(screen.getByText('연결됨')).toBeInTheDocument();
  expect(screen.getByText('115200')).toBeInTheDocument();
  // What the board is, and the operator's note for it (the COM port).
  expect(screen.getByText('COM5')).toBeInTheDocument();
  expect(screen.getByText('USB 303A:1001 · Espressif USB-JTAG/Serial (ESP32-C3/S3/C6/H2)')).toBeInTheDocument();
  expect(document.title).toBe('esp32 (COM5) · 시리얼 모니터');

  const input = screen.getByRole('textbox');
  fireEvent.change(input, { target: { value: 'AT' } });
  fireEvent.keyDown(input, { key: 'Enter' });
  await waitFor(() => expect(asked.some((m) => m.type === 'send')).toBe(true), { timeout: 5000 });
  expect(new TextDecoder().decode(asked.find((m) => m.type === 'send').bytes)).toBe('AT\n');
  // ↑ brings the last line back.
  fireEvent.keyDown(input, { key: 'ArrowUp' });
  expect(input).toHaveValue('AT');

  fireEvent.click(screen.getByRole('button', { name: '보드 리셋' }));
  await waitFor(() => expect(asked.some((m) => m.type === 'reset')).toBe(true), { timeout: 5000 });
}, 20000);

it('says when no tab holds the port, and picks up once one answers', async () => {
  view('pty_2', 'esp32');
  expect(await screen.findByText('포트를 잡은 창 없음', {}, { timeout: 8000 })).toBeInTheDocument();
  const holder = holderFor('pty_2', 'esp32');
  holder.onmessage = (e: any) => { if (e.data.type === 'hello') holder.postMessage({ type: 'state', state: 'waiting', baud: 9600, path: '' }); };
  // The window asks again every 3 s; under a loaded test run that can take a while.
  expect(await screen.findByText('보드 재연결 대기 중…', {}, { timeout: 10000 })).toBeInTheDocument();
  expect(screen.getByRole('textbox')).toBeDisabled();
}, 25000);

it('the tab holding the port answers a monitor and carries out what it asks', async () => {
  const bridge = {
    state: { status: 'connected', socketUp: true, portUp: true, path: '/s/esp32', error: null, stuck: null, log: 'boot\r\n' } as any,
    send: jest.fn(async (_bytes: Uint8Array) => true),
    reset: jest.fn(async () => undefined),
    bootloader: jest.fn(async () => { throw new Error('port busy'); }),
  };
  const holder = monitorChannel('pty_3', 'esp32', 115200, () => bridge)!;
  const monitor = holderFor('pty_3', 'esp32');
  const heard: any[] = [];
  monitor.onmessage = (e: any) => heard.push(e.data);
  monitor.postMessage({ type: 'hello' });
  monitor.postMessage({ type: 'send', bytes: new Uint8Array([65, 10]) });
  monitor.postMessage({ type: 'reset' });
  monitor.postMessage({ type: 'bootloader' });
  await waitFor(() => expect(heard.filter((m) => m.type === 'done')).toHaveLength(3), { timeout: 5000 });
  expect(heard).toEqual(expect.arrayContaining([
    { type: 'backlog', text: 'boot\r\n' },
    { type: 'state', state: 'connected', baud: 115200, path: '/s/esp32', device: '', label: '' },
    { type: 'done', what: 'send', ok: true },
    { type: 'done', what: 'reset', ok: true },
    { type: 'done', what: 'bootloader', ok: false, error: 'port busy' },
  ]));
  expect(Array.from(bridge.send.mock.calls[0][0])).toEqual([65, 10]);
  holder.close();
}, 20000);

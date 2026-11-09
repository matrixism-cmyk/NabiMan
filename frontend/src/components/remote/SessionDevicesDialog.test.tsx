import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { TextDecoder } from 'util';
import { I18nProvider } from '../../i18n';
import SessionDevicesDialog from './SessionDevicesDialog';
import * as devices from './serialDevices';
import { apiRequest } from '../../shared/api/useApi';
import { sessionSerialTicket } from './vncApi';

jest.mock('./vncApi', () => ({ sessionSerialTicket: jest.fn() }));
jest.mock('../../shared/api/useApi', () => ({ apiRequest: jest.fn() }));
(global as any).TextDecoder = TextDecoder;

class FakeSocket {
  static OPEN = 1;
  readyState = 0;
  binaryType = '';
  onopen: (() => void) | null = null;
  onmessage: ((e: unknown) => void) | null = null;
  onclose: ((e: unknown) => void) | null = null;
  constructor(readonly url: string) { /* never opens: the dialog only needs the device listed */ }
  send() { /* nothing */ }
  close() { this.readyState = 3; }
}

/** A CP210x board that says nothing. Made per test: the suite resets mock implementations. */
function makePort() {
  const p: any = {
    readable: null,
    writable: null,
    getInfo: () => ({ usbVendorId: 0x10c4, usbProductId: 0xea60 }),
    open: jest.fn(async () => {
      // Quiet until cancelled, as a real reader is when the port is closed.
      let cancel: () => void = () => {};
      p.readable = { getReader: () => ({
        read: () => new Promise((resolve) => { cancel = () => resolve({ done: true }); }),
        cancel: async () => cancel(), releaseLock: () => {},
      }) };
      p.writable = { getWriter: () => ({ write: async () => {}, releaseLock: () => {} }) };
    }),
    close: jest.fn(async () => {}),
    setSignals: jest.fn(async () => {}),
  };
  return p;
}

beforeEach(() => {
  const port = makePort();
  localStorage.clear();
  localStorage.setItem('nabiman_lang', 'ko');
  (sessionSerialTicket as jest.Mock).mockResolvedValue('a'.repeat(64));
  (apiRequest as jest.Mock).mockResolvedValue({ success: true, data: [] });
  (global as any).WebSocket = FakeSocket;
  (navigator as any).serial = { requestPort: jest.fn(async () => port), getPorts: jest.fn(async () => [port]) };
});
afterEach(async () => { for (const d of devices.getDevices()) await devices.detach(d.sid, d.name); });

it('shows what each board is and keeps a note (the COM port) the operator types', async () => {
  expect(await devices.attach('pty_d', 'esp32', 115200)).toBeNull();
  render(<I18nProvider><SessionDevicesDialog sessionId="pty_d" sessionName="iot" onClose={jest.fn()} /></I18nProvider>);
  expect(screen.getByText('USB 10C4:EA60 · Silicon Labs CP210x')).toBeInTheDocument();
  const note = screen.getByRole('textbox', { name: 'esp32 메모' });
  fireEvent.change(note, { target: { value: 'COM7' } });
  fireEvent.blur(note);
  await waitFor(() => expect(devices.getDevices()[0].label).toBe('COM7'));
  // The note sits beside the name too, and each device has its monitor button.
  expect(await screen.findByText('COM7')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '모니터 ⧉' })).toBeInTheDocument();
});

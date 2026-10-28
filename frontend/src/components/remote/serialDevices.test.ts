import { TextDecoder } from 'util';
import * as devices from './serialDevices';
import { sessionSerialTicket } from './vncApi';

jest.mock('./vncApi', () => ({ sessionSerialTicket: jest.fn() }));
(global as any).TextDecoder = TextDecoder;

class FakeSocket {
  static all: FakeSocket[] = [];
  static OPEN = 1;
  readyState = 0;
  binaryType = '';
  onopen: (() => void) | null = null;
  onmessage: ((e: { data: unknown }) => void) | null = null;
  onclose: ((e: { code: number; reason: string }) => void) | null = null;
  sent: unknown[] = [];
  constructor(readonly url: string) { FakeSocket.all.push(this); }
  send(d: unknown) { this.sent.push(d); }
  close() { this.readyState = 3; }
  open() { this.readyState = 1; this.onopen?.(); }
}

/** A Web Serial port whose reader waits until cancelled. */
function fakePort(vendor = 0x303a, product = 0x1001) {
  const port: any = {
    readable: null, writable: null, signals: [] as unknown[],
    getInfo: () => ({ usbVendorId: vendor, usbProductId: product }),
    open: jest.fn(async () => {
      let cancel: () => void = () => {};
      port.readable = { getReader: () => ({
        read: () => new Promise((resolve) => { cancel = () => resolve({ done: true }); }),
        cancel: async () => cancel(), releaseLock: () => {},
      }) };
      port.writable = { getWriter: () => ({ write: jest.fn(async () => {}), releaseLock: () => {} }) };
    }),
    close: jest.fn(async () => { port.readable = port.writable = null; }),
    setSignals: jest.fn(async (s: unknown) => { port.signals.push(s); }),
  };
  return port;
}

const flush = () => new Promise((r) => setTimeout(r, 0));
let ports: any[];

beforeEach(() => {
  jest.useRealTimers();
  localStorage.clear();
  (sessionSerialTicket as jest.Mock).mockResolvedValue('a'.repeat(64));
  FakeSocket.all = [];
  ports = [fakePort()];
  (global as any).WebSocket = FakeSocket;
  (navigator as any).serial = { requestPort: jest.fn(async () => ports[0]), getPorts: jest.fn(async () => ports) };
});
afterEach(async () => {
  for (const d of devices.getDevices()) await devices.detach(d.sid, d.name);
});

test('attaching opens the port, dials the session socket and remembers the device', async () => {
  expect(await devices.attach('pty_1', 'esp32', 115200)).toBeNull();
  await flush(); await flush();
  expect(ports[0].open).toHaveBeenCalledWith({ baudRate: 115200 });
  expect(FakeSocket.all[0].url).toMatch(/\/api\/terminal\/serial\?session_id=pty_1&serial=esp32&ticket=a{64}$/);
  FakeSocket.all[0].open();
  expect(devices.getDevices()).toEqual([expect.objectContaining({ sid: 'pty_1', name: 'esp32', socketUp: true, portUp: true })]);
  expect(JSON.parse(localStorage.getItem('nabiman_serial_devices')!)).toEqual({ pty_1: [{ name: 'esp32', baud: 115200, device: '12346:4097' }] });
});

test('a name is taken once per session, and checked', async () => {
  await devices.attach('pty_1', 'esp32', 115200);
  expect(await devices.attach('pty_1', 'esp32', 115200)).toEqual({ key: 'serial.dev.nameTaken', vars: { name: 'esp32' } });
  expect(await devices.attach('pty_1', 'Bad Name', 115200)).toEqual({ key: 'serial.dev.badName' });
  expect(devices.suggestName(['esp32', 'esp32-2'])).toBe('esp32-3');
});

test('a remembered device comes back with no picker, and detaching forgets it', async () => {
  localStorage.setItem('nabiman_serial_devices', JSON.stringify({ pty_9: [{ name: 'board', baud: 9600, device: '12346:4097' }] }));
  devices.resumeSession('pty_9');
  await flush(); await flush(); await flush();
  expect((navigator as any).serial.requestPort).not.toHaveBeenCalled();
  expect(ports[0].open).toHaveBeenCalledWith({ baudRate: 9600 });
  expect(devices.getDevices().map((d) => d.name)).toEqual(['board']);
  await devices.detach('pty_9', 'board');
  expect(devices.getDevices()).toEqual([]);
  expect(localStorage.getItem('nabiman_serial_devices')).toBe('{}');
});

test('ended sessions take their devices and memory with them', async () => {
  await devices.attach('pty_1', 'esp32', 115200);
  localStorage.setItem('nabiman_serial_devices', JSON.stringify({
    ...JSON.parse(localStorage.getItem('nabiman_serial_devices')!), pty_gone: [{ name: 'x', baud: 1, device: '1:1' }],
  }));
  devices.pruneSessions(new Set(['pty_other']));
  await flush();
  expect(devices.getDevices()).toEqual([]);
  expect(localStorage.getItem('nabiman_serial_devices')).toBe('{}');
});

test('a server signal drives the lines one at a time and is answered', async () => {
  await devices.attach('pty_1', 'esp32', 115200);
  await flush(); await flush();
  const socket = FakeSocket.all[0];
  socket.open();
  socket.onmessage?.({ data: JSON.stringify({ signal: 'reset' }) });
  await new Promise((r) => setTimeout(r, 200));
  expect(ports[0].signals).toEqual([{ dataTerminalReady: false }, { requestToSend: true }, { requestToSend: false }]);
  expect(socket.sent).toContain(JSON.stringify({ ack: 'reset' }));
});

test('a board that cannot be opened again says why, and can be chosen again', async () => {
  localStorage.setItem('nabiman_serial_devices', JSON.stringify({ pty_5: [{ name: 'esp32', baud: 115200, device: '12346:4097' }] }));
  const granted = ports;
  ports = [];                                   // plugged back in, and the grant went with it
  devices.resumeSession('pty_5');
  await flush(); await flush();
  expect(devices.getDevices()[0]).toEqual(expect.objectContaining({ portUp: false, stuck: { key: 'serial.dev.notGranted' } }));

  ports = [fakePort()];                         // listed, but held by another program
  ports[0].open.mockRejectedValueOnce(new Error('Failed to open serial port.'));
  await new Promise((r) => setTimeout(r, 1100));
  expect(devices.getDevices()[0].stuck).toEqual({ key: 'serial.dev.openFailed', vars: { text: 'Failed to open serial port.' } });

  ports = granted;
  (navigator as any).serial.getPorts = jest.fn(async () => []);
  expect(await devices.repick('pty_5', 'esp32')).toBeNull();
  expect(devices.getDevices()[0]).toEqual(expect.objectContaining({ portUp: true, stuck: null }));
});

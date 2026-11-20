import { RdpSession } from './rdpSession';

const mockClients: any[] = [];

jest.mock('../../vendor/guacamole-common.js', () => {
  class Client {
    state = 0;
    connected: string | null = null;
    sizes: [number, number][] = [];
    element = global.document.createElement('div');
    onstatechange: ((state: number) => void) | null = null;
    constructor() { mockClients.push(this); }
    screen = [0, 0];
    /** Sizes the VM will ignore before it starts following (Windows, right after connecting). */
    deaf = 0;
    getDisplay() { return { getElement: () => this.element, getWidth: () => this.screen[0], getHeight: () => this.screen[1], getScale: () => 1, scale: () => undefined, onresize: null }; }
    connect(query: string) { this.connected = query; }
    disconnect() { /* gone */ }
    /** As Guacamole: a size sent before the connection is up is dropped. */
    sendSize(w: number, h: number) { if (this.state !== 3) return; this.sizes.push([w, h]); if (this.deaf > 0) this.deaf -= 1; else this.screen = [w - (w % 2), h]; }
    sendMouseState() { /* not looked at */ }
    come() { this.state = 3; const m = /GUAC_WIDTH=(\d+)&GUAC_HEIGHT=(\d+)/.exec(this.connected || ''); if (m) this.screen = [Number(m[1]), Number(m[2])]; this.onstatechange?.(3); }
  }
  class Pointer { }
  const Mouse: any = Pointer;
  Mouse.State = class {}; Mouse.Touchpad = Pointer; Mouse.Touchscreen = Pointer;
  return { __esModule: true, default: { Client, WebSocketTunnel: class {}, Mouse } };
});
jest.mock('./vncApi', () => ({ rdpTicket: () => Promise.resolve('f'.repeat(64)), consoleCreds: () => Promise.resolve('creds') }));

const host = (w: number, h: number) => {
  const el = document.createElement('div');
  Object.defineProperty(el, 'clientWidth', { value: w }); Object.defineProperty(el, 'clientHeight', { value: h });
  return el;
};

beforeEach(() => { mockClients.length = 0; jest.useFakeTimers(); });
afterEach(() => { jest.useRealTimers(); });

async function started(w: number, h: number) {
  const session = new RdpSession('vm-1');
  session.attach(host(w, h));
  session.start();
  await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  return { session, client: mockClients[mockClients.length - 1] };
}

test('room that changed while connecting is told to the VM once it is up', async () => {
  const { session, client } = await started(1182, 699);
  expect(client.connected).toContain('GUAC_WIDTH=1182&GUAC_HEIGHT=699');
  // The bar wraps onto a second row before the connection is up.
  session.resize(1182, 662);
  jest.advanceTimersByTime(500);
  expect(client.sizes).toEqual([]);
  client.come();
  jest.advanceTimersByTime(10);
  expect(client.sizes).toEqual([[1182, 662]]);
  jest.advanceTimersByTime(10000);
  expect(client.sizes).toEqual([[1182, 662]]);
  session.close();
});

test('a size the VM ignored right after connecting is told again until the screen follows', async () => {
  const { session, client } = await started(1182, 699);
  client.deaf = 1;
  client.come();
  session.resize(1182, 662);
  jest.advanceTimersByTime(400);
  expect(client.sizes).toEqual([[1182, 662]]);
  expect(client.screen).toEqual([1182, 699]);
  jest.advanceTimersByTime(3000);
  expect(client.sizes).toEqual([[1182, 662], [1182, 662]]);
  expect(client.screen).toEqual([1182, 662]);
  jest.advanceTimersByTime(20000);
  expect(client.sizes.length).toBe(2);
  session.close();
});

test('a VM that never follows is not asked for ever', async () => {
  const { session, client } = await started(1182, 699);
  client.deaf = 99;
  client.come();
  session.resize(1182, 662);
  jest.advanceTimersByTime(60000);
  expect(client.sizes.length).toBe(4);
  session.close();
});

test('an unchanged room sends nothing; a changed one is sent once, after the storm', async () => {
  const { session, client } = await started(1182, 699);
  client.come();
  jest.advanceTimersByTime(500);
  expect(client.sizes).toEqual([]);
  session.resize(1182, 699);
  session.resize(1300, 720); session.resize(1400, 760); session.resize(1500, 800);
  jest.advanceTimersByTime(500);
  expect(client.sizes).toEqual([[1500, 800]]);
  session.close();
});

test('a fixed desktop size is never changed by the room', async () => {
  const session = new RdpSession('vm-1');
  session.configure({ fixed: { w: 1024, h: 768 }, speed: 'balanced' });
  session.attach(host(1182, 699));
  session.start();
  await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  const client = mockClients[mockClients.length - 1];
  expect(client.connected).toContain('GUAC_WIDTH=1024&GUAC_HEIGHT=768');
  client.come();
  session.resize(1300, 720);
  jest.advanceTimersByTime(500);
  expect(client.sizes).toEqual([]);
  session.close();
});

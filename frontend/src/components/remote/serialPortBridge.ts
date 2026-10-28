/**
 * One serial bridge: a Web Serial port in this browser (an ESP32 on the
 * operator's USB, say) relayed to a NabiMan WebSocket, which exposes it to
 * server tools as a PTY. Kept outside React so it outlives the view showing it.
 *
 * Both halves heal on their own. A board on native USB (ESP32-C3 SuperMini…)
 * drops off the bus whenever the chip resets, so a lost port is reopened when
 * the same device comes back; a dropped socket (server restart) is redialled.
 * The server can ask for `reset` / `bootloader` — a PTY carries no DTR/RTS —
 * and the bridge drives the lines and answers.
 */

export type BridgeStatus = 'idle' | 'connecting' | 'connected';
export type Notice = { key: string; vars?: Record<string, string | number> };
export interface BridgeState {
  status: BridgeStatus;
  socketUp: boolean;
  portUp: boolean;
  path: string;
  error: Notice | null;
  /** Why the board cannot be opened again, while waiting for it. */
  stuck: Notice | null;
  log: string;
}

const REATTACH_EVERY_MS = 1000;
const REDIAL_MS = [1000, 2000, 5000, 10000];

export const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
/** USB vendor:product, so the same board is found again after it re-enumerates. */
export const portKey = (p: any) => { const i = p?.getInfo?.() ?? {}; return `${i.usbVendorId ?? ''}:${i.usbProductId ?? ''}`; };
/** Espressif's built-in USB-Serial/JTAG (ESP32-C3/S3/C6…): DTR is the boot pin, RTS the reset. */
const isUsbJtag = (p: any) => { const i = p?.getInfo?.() ?? {}; return i.usbVendorId === 0x303a && i.usbProductId === 0x1001; };
export const serialApi = (): any => (typeof navigator !== 'undefined' ? (navigator as any).serial : undefined);
export const serialSupported = () => !!serialApi();

let translate: (key: string, vars?: Record<string, string | number>) => string = (k) => k;
export const setTranslator = (t: typeof translate) => { translate = t; };
export const noticeText = (n: Notice) => translate(n.key, n.vars);

/** Drive DTR/RTS one line per step, as esptool does: drivers (Windows usbser)
 *  apply a two-line change one line at a time, which can release the boot pin mid-reset. */
type Step = { dtr?: boolean; rts?: boolean; ms?: number };
const RESET: Step[] = [{ dtr: false }, { rts: true, ms: 120 }, { rts: false }];
// esptool USBJTAGSerialReset: hold boot (DTR), then go to reset through (1,1), never (0,0).
const BOOT_USB_JTAG: Step[] = [{ rts: false }, { dtr: false, ms: 100 }, { dtr: true }, { rts: false, ms: 100 },
  { rts: true }, { dtr: false }, { rts: true, ms: 100 }, { dtr: false }, { rts: false }];
// esptool ClassicReset (CP210x/CH340 auto-reset circuit).
const BOOT_CLASSIC: Step[] = [{ dtr: false }, { rts: true, ms: 100 }, { dtr: true }, { rts: false, ms: 50 }, { dtr: false }];

export interface BridgeOptions {
  baud: number;
  /** `portKey` of the board, to find it again among the ports this page may use. */
  device: string;
  /** A fresh, ticketed WebSocket URL for each dial. */
  url: () => Promise<string>;
  onChange: () => void;
  /** Web Locks name: one tab of this browser bridges the device, as the server takes one socket for it. */
  lock?: string;
}

export class PortBridge {
  state: BridgeState = { status: 'idle', socketUp: false, portUp: false, path: '', error: null, stuck: null, log: '' };
  /** `portKey` of the board; a board chosen again (`adopt`) replaces it. */
  device: string;
  private port: any = null;
  private reader: any = null;
  private writer: any = null;
  private pumping: Promise<void> | null = null;
  private ws: WebSocket | null = null;
  private running = false;  // between start and stop: both halves should be up
  private reattaching = false;
  private reattachTimer: ReturnType<typeof setInterval> | null = null;
  private redialTimer: ReturnType<typeof setTimeout> | null = null;
  private redials = 0;
  private unlock: (() => void) | null = null;
  private lockWait: AbortController | null = null;

  constructor(readonly opts: BridgeOptions) { this.device = opts.device; }

  private set(patch: Partial<BridgeState>) { this.state = { ...this.state, ...patch }; this.opts.onChange(); }
  /** Says why the board is not back, only when that changes (reattach runs every second). */
  private stuck(n: Notice | null) { if (JSON.stringify(n) !== JSON.stringify(this.state.stuck)) this.set({ stuck: n }); }
  private append(s: string) { this.set({ log: (this.state.log.length > 20000 ? this.state.log.slice(-16000) : this.state.log) + s }); }
  private note(n: Notice) { this.append(`\r\n[${noticeText(n)}]\r\n`); }

  /** Bridge a port the operator just chose (already through `requestPort`). */
  async startWith(chosen: any) {
    this.set({ status: 'connecting', error: null });
    this.running = true;
    if (!(await this.hold(false))) {
      this.running = false;
      this.set({ status: 'idle', error: { key: 'serial.dev.otherTabBusy' } });
      return false;
    }
    try {
      await this.openPort(chosen);
    } catch (e) {
      await this.stop();
      this.set({ error: { key: 'serial.raw', vars: { text: errText(e) } } });
      return false;
    }
    this.begin();
    return true;
  }

  /** Bridge the remembered board again without a picker (page load, after a server restart). */
  async resume() {
    if (this.running || !serialSupported()) return;
    this.running = true;
    this.set({ status: 'connecting', error: null });
    if (!(await this.hold(true))) return;
    this.stuck(null);
    this.begin();
    await this.reattach(false);
  }

  /**
   * Take the device's lock, so that a second tab of the same session does not
   * fight the first for the server's one socket (and for the port). Without
   * `wait` a held lock is a refusal; with it the tab waits its turn, saying
   * so, and takes over when the other tab closes. Resolves to whether to go on.
   */
  private hold(wait: boolean): Promise<boolean> {
    const locks = (navigator as any).locks;
    const name = this.opts.lock;
    if (!name || !locks) return Promise.resolve(true);
    return new Promise((done) => {
      const keep = () => {
        // Granted after a stop: let it go at once.
        if (!this.running) { done(false); return undefined; }
        done(true);
        return new Promise<void>((r) => { this.unlock = r; });
      };
      locks.request(name, { ifAvailable: true }, (held: unknown) => {
        if (held) return keep();
        if (!wait) { done(false); return undefined; }
        this.stuck({ key: 'serial.dev.otherTab' });
        this.lockWait = new AbortController();
        locks.request(name, { signal: this.lockWait.signal }, keep).catch(() => done(false));
        return undefined;
      }).catch(() => done(true));
    });
  }

  private begin() {
    this.running = true;
    this.redials = 0;
    this.set({ status: 'connecting', error: null });
    this.reattachTimer = setInterval(() => { void this.reattach(true); }, REATTACH_EVERY_MS);
    void this.dial();
  }

  async stop() {
    this.running = false;
    if (this.reattachTimer) clearInterval(this.reattachTimer);
    if (this.redialTimer) clearTimeout(this.redialTimer);
    this.reattachTimer = this.redialTimer = null;
    const socket = this.ws; const p = this.port;
    this.ws = this.port = null;
    try { socket?.close(); } catch { /* ignore */ }
    try { await this.reader?.cancel(); } catch { /* ignore */ }
    try { await this.pumping; } catch { /* ignore */ }
    try { this.writer?.releaseLock(); } catch { /* ignore */ }
    this.reader = this.writer = this.pumping = null;
    try { await p?.close(); } catch { /* ignore */ }
    // The port is closed, so a tab waiting its turn can open it.
    this.lockWait?.abort(); this.unlock?.();
    this.lockWait = null; this.unlock = null;
    this.set({ status: 'idle', socketUp: false, portUp: false, path: '', stuck: null });
  }

  /** A board the operator chose again in the picker (its grant was lost, say): open it now. */
  async adopt(chosen: any) {
    if (this.port) return;
    this.device = portKey(chosen);
    await this.openPort(chosen);
    this.stuck(null);
    this.note({ key: 'serial.portBack' });
  }

  reset() { return this.lines(RESET); }
  bootloader() { return this.lines(isUsbJtag(this.port) ? BOOT_USB_JTAG : BOOT_CLASSIC); }

  private async lines(steps: Step[]) {
    const p = this.port;
    if (!p) throw new Error(translate('serial.status.waiting'));
    for (const { dtr, rts, ms } of steps) {
      if (dtr !== undefined) await p.setSignals({ dataTerminalReady: dtr });
      if (rts !== undefined) await p.setSignals({ requestToSend: rts });
      if (ms) await sleep(ms);
    }
  }

  /** Read the port until it goes away, relaying to the socket; then wait for it to come back. */
  private async pump(p: any) {
    const dec = new TextDecoder();
    let reason = '';
    // A non-fatal error (framing, overrun) hands back a fresh readable; a lost device leaves none.
    while (p.readable && this.port === p) {
      const rd = p.readable.getReader();
      this.reader = rd;
      try {
        for (;;) {
          const { value, done } = await rd.read();
          if (done) break;
          if (!value?.length) continue;
          if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(value);
          this.append(dec.decode(value, { stream: true }));
        }
      } catch (e) {
        reason = errText(e);
      } finally {
        try { rd.releaseLock(); } catch { /* ignore */ }
      }
    }
    if (!this.running || this.port !== p) return;
    try { this.writer?.releaseLock(); } catch { /* ignore */ }
    this.reader = this.writer = this.port = null;
    try { await p.close(); } catch { /* already gone */ }
    this.set({ portUp: false });
    this.note({ key: 'serial.portLost', vars: { reason: reason || '-' } });
  }

  private async openPort(p: any) {
    await p.open({ baudRate: this.opts.baud });
    if (!p.readable || !p.writable) {
      try { await p.close(); } catch { /* ignore */ }
      throw new Error(translate('serial.portGone'));
    }
    this.port = p;
    this.writer = p.writable.getWriter();
    this.set({ portUp: true, stuck: null });
    this.pumping = this.pump(p);
  }

  /** Find the remembered board among the ports this page may use and open it. */
  private async reattach(announce: boolean) {
    if (!this.running || this.port || this.reattaching) return;
    this.reattaching = true;
    try {
      const ports: any[] = (await serialApi().getPorts()).filter((p: any) => portKey(p) === this.device);
      let why = '';
      for (const p of ports) {
        if (p.readable) continue;
        try {
          await this.openPort(p);
          if (announce) this.note({ key: 'serial.portBack' });
          return;
        } catch (e) { why = errText(e); /* still enumerating, or held by another app — next tick */ }
      }
      // Not among the ports this page may use: unplugged, or the grant went with it.
      this.stuck(!ports.length ? { key: 'serial.dev.notGranted' } : why ? { key: 'serial.dev.openFailed', vars: { text: why } } : null);
    } catch { /* ignore */ } finally {
      this.reattaching = false;
    }
  }

  /** A signal the server asked for: wait briefly for the port if it is mid-reattach, run it, answer. */
  private async serverSignal(socket: WebSocket, signal: string) {
    const run = signal === 'reset' ? () => this.reset() : signal === 'bootloader' ? () => this.bootloader() : null;
    if (!run) return;
    let error = '';
    for (let i = 0; i < 50 && !this.port; i++) await sleep(100);
    try { await run(); this.note({ key: 'serial.serverSignal', vars: { signal } }); } catch (e) { error = errText(e); }
    if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(error ? { ack: signal, error } : { ack: signal }));
  }

  private async dial() {
    if (!this.running || this.ws) return;
    let url: string;
    try {
      url = await this.opts.url();
    } catch (e) {
      this.set({ error: { key: 'serial.wsClosed', vars: { code: '-', reason: errText(e) } } });
      this.scheduleRedial();
      return;
    }
    if (!this.running || this.ws) return;
    const socket = new WebSocket(url);
    socket.binaryType = 'arraybuffer';
    this.ws = socket;
    socket.onopen = () => { this.redials = 0; this.set({ status: 'connected', socketUp: true, error: null }); };
    socket.onmessage = (e) => {
      if (typeof e.data === 'string') {
        try {
          const m = JSON.parse(e.data);
          if (m.path) this.set({ path: m.path });
          if (m.signal) void this.serverSignal(socket, m.signal);
        } catch { /* not ours */ }
        return;
      }
      this.writer?.write(new Uint8Array(e.data)).catch(() => { /* port gone; reattach picks it up */ });
    };
    socket.onclose = (e) => {
      if (this.ws !== socket) return;
      this.ws = null;
      this.set({ socketUp: false, path: '' });
      if (!this.running) return;
      this.set({ error: { key: 'serial.wsClosed', vars: { code: e.code, reason: e.reason || '-' } } });
      this.note({ key: 'serial.redialing' });
      this.scheduleRedial();
    };
  }

  private scheduleRedial() {
    if (!this.running || this.redialTimer) return;
    const ms = REDIAL_MS[Math.min(this.redials++, REDIAL_MS.length - 1)];
    this.redialTimer = setTimeout(() => { this.redialTimer = null; void this.dial(); }, ms);
  }
}

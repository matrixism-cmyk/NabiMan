import Guacamole from '../../vendor/guacamole-common.js';
import { Attempt, ConsoleSession } from './consoleSession';
import type { ConsoleShare } from './consoleView';
import { ConsoleSpeed, DEFAULT_SPEED } from './consoleSpeed';
import { saveBlob } from './consoleDownload';
import { rdpZoom } from './rdpZoom';
import { consoleCreds, rdpTicket } from './vncApi';

/** What the view chose for the connection; changing any of it reconnects. */
export interface RdpOptions { touch?: 'screen' | 'pad'; fixed: { w: number; h: number } | null; speed: ConsoleSpeed }

const sameOptions = (a: RdpOptions, b: RdpOptions) =>
  a.touch === b.touch && a.speed === b.speed && a.fixed?.w === b.fixed?.w && a.fixed?.h === b.fixed?.h;

/**
 * One RDP console connection over NabiMan's guacd tunnel, apart from the view
 * that shows it: the Guacamole client, the screen it draws (`stage`, which a
 * view puts on the page while it is up) and the pointer on that screen. The
 * keyboard is the view's, bound to the page only while the console shows, so
 * a console kept out of sight takes no keys.
 */
export class RdpSession extends ConsoleSession {
  /** The screen's own scrolling box; holds Guacamole's display. */
  readonly stage = document.createElement('div');
  client: any = null;
  filesystem: any = null;
  /** 1 is the whole screen fitted in. */
  zoom = 1;
  /** A file the VM sent down was saved (the view says so). */
  onReceived?: (name: string) => void;
  private options: RdpOptions = { fixed: null, speed: DEFAULT_SPEED };
  private size = { w: 1280, h: 800 };
  private alt: { user: string; password: string } | null = null;
  private zoomer: ReturnType<typeof rdpZoom> | null = null;
  private sizeTimer?: ReturnType<typeof setTimeout>;

  constructor(readonly target: string, readonly share?: ConsoleShare) {
    super();
    this.stage.className = 'rw-rdp-stage';
    // A phone's screen is taller than the desktop fitted into it; a tap on the
    // empty space below is nothing, rather than the browser focusing the page there.
    this.stage.addEventListener('touchstart', (e) => { if (e.target === this.stage) e.preventDefault(); }, { passive: false });
  }

  /** The settings to connect with. Different ones reconnect a running session. */
  configure(next: RdpOptions) {
    if (sameOptions(next, this.options)) return;
    this.options = next;
    if (this.running()) this.reconnect();
  }

  /** Another account for this console (a multi-user VM), from the next connect on; kept in memory only. */
  setAccount(user: string, password: string) { this.alt = { user, password }; }
  altUser() { return this.alt?.user || null; }

  /** A view puts the screen on the page, in `host`. */
  attach(host: HTMLElement) {
    host.insertBefore(this.stage, host.firstChild);
    this.attached = true;
    this.resize(host.clientWidth, host.clientHeight);
  }

  /** The view lets go; the connection stays. */
  detach() {
    this.attached = false;
    this.onReceived = undefined;
    this.stage.remove();
  }

  fit() { this.zoomer?.fit(); }
  resetZoom() { this.zoomer?.reset(); }

  /** The room the view has. Unless the desktop's size is fixed, the VM is asked
   *  to match it (resize-method display-update), so it stays crisp at 1:1.
   *  Debounced against resize storms; guacd's resize then fits the screen. */
  resize(width: number, height: number) {
    this.fit();
    if (!width || !height) return;
    const w = Math.max(width, 640), h = Math.max(height, 480);
    if (w === this.size.w && h === this.size.h) return;
    this.size = { w, h };
    if (this.options.fixed) return;
    if (this.sizeTimer) clearTimeout(this.sizeTimer);
    this.sizeTimer = setTimeout(() => { try { this.client?.sendSize(w, h); } catch { /* not connected yet */ } }, 300);
  }

  sendKey(down: boolean, keysym: number) { try { this.client?.sendKeyEvent(down ? 1 : 0, keysym); } catch { /* not ready */ } }

  /** Gives the VM the local clipboard, so Ctrl+V inside it pastes what was copied here. */
  pushClipboard = () => {
    const c = this.client; if (!c || !this.attached) return;
    navigator.clipboard.readText().then((text) => {
      if (!text || c !== this.client) return;
      const writer = new Guacamole.StringWriter(c.createClipboardStream('text/plain'));
      writer.sendText(text); writer.sendEnd();
    }).catch(() => { /* no permission */ });
  };

  protected open({ live, fail }: Attempt) {
    const { share, stage, options } = this;
    const pad = options.touch === 'pad';
    // From the site root: relative, a share page (/cshare/<token>) reached /cshare/api/….
    const tunnel = new Guacamole.WebSocketTunnel(share ? `/api/cshare/${encodeURIComponent(share.token)}/ws` : '/api/remote/rdp');
    const c = new Guacamole.Client(tunnel);
    this.client = c;
    const display = c.getDisplay();
    stage.innerHTML = '';
    stage.appendChild(display.getElement());

    // Fitted whole, or as far as two fingers zoomed in (rdpZoom).
    let touch: any = null;
    this.zoom = 1;
    const zoomed = rdpZoom({
      stage, display, pinchOnly: pad, onZoom: (z) => { if (live()) { this.zoom = z; this.emit(); } },
      onRescale: (ratio) => { if (pad && touch) { touch.currentState.x *= ratio; touch.currentState.y *= ratio; } },
    });
    this.zoomer = zoomed;
    display.onresize = zoomed.fit;

    c.onstatechange = (state: number) => {
      if (!live()) return;
      if (state === 3) { this.up(); zoomed.fit(); } else if (state === 5) fail();
    };
    c.onerror = (err: { code?: number; message?: string }) => fail(err);
    // Text copied inside the VM → the local clipboard, while the console shows.
    c.onclipboard = (stream: any, mimetype: string) => {
      if (mimetype.indexOf('text') !== 0) return;
      const reader = new Guacamole.StringReader(stream);
      let text = '';
      reader.ontext = (t2: string) => { text += t2; };
      reader.onend = () => { if (this.attached) navigator.clipboard.writeText(text).catch(() => { /* clipboard refused: the text stays on screen to select */ }); };
    };
    // guacd's redirected drive becomes available — keep it for file transfer.
    c.onfilesystem = (object: any) => { if (live()) { this.filesystem = object; this.emit(); } };
    // A file the VM puts in the drive's Download folder: guacd sends it here to save.
    c.onfile = (stream: any, mimetype: string, filename: string) => {
      if (share?.viewOnly) { stream.sendAck('Refused', 0x0303); return; }
      const reader = new Guacamole.BlobReader(stream, mimetype);
      reader.onend = () => { saveBlob(reader.getBlob(), filename); this.onReceived?.(filename); };
      stream.sendAck('Ready', 0);
    };

    const { w, h } = options.fixed || this.size;
    const query = `GUAC_WIDTH=${w}&GUAC_HEIGHT=${h}&GUAC_DPI=96&speed=${options.speed}`;
    if (share) {
      c.connect((share.pw ? `pw=${encodeURIComponent(share.pw)}&` : '') + query);
    } else {
      (async () => {
        try {
          const ticket = await rdpTicket(this.target);
          // An alternate account issues a one-use creds token for this connect.
          const creds = this.alt ? `&creds=${await consoleCreds(this.target, this.alt.user, this.alt.password)}` : '';
          if (live()) c.connect(`target=${encodeURIComponent(this.target)}&ticket=${ticket}&${query}${creds}`);
        } catch (e) { fail({ message: e instanceof Error ? e.message : undefined }); }
      })();
    }

    // The pointer (none at all for a view-only share).
    if (share?.viewOnly) return;
    // Pointer positions are on the scaled screen; the VM wants its own pixels.
    const send = (s: any) => {
      const k = display.getScale() || 1;
      c.sendMouseState(new Guacamole.Mouse.State(s.x / k, s.y / k, s.left, s.middle, s.right, s.up, s.down));
    };
    const mouse = new Guacamole.Mouse(display.getElement());
    mouse.onmousedown = mouse.onmouseup = mouse.onmousemove = send;
    // Guacamole.Mouse ignores touches, so a phone gets its own: tap where you
    // touch (long press: right click), or a touchpad that moves the pointer.
    touch = pad ? new Guacamole.Mouse.Touchpad(display.getElement()) : new Guacamole.Mouse.Touchscreen(display.getElement());
    touch.onmousedown = touch.onmouseup = send;
    // A touchpad's pointer can leave a zoomed-in view; the view goes after it.
    touch.onmousemove = pad ? (s: any) => { send(s); zoomed.follow(s.x, s.y); } : send;
  }

  protected shut() {
    if (this.sizeTimer) clearTimeout(this.sizeTimer);
    this.zoomer?.detach();
    this.zoomer = null;
    const c = this.client;
    this.client = null;
    this.filesystem = null;
    try { c?.disconnect(); } catch { /* already gone */ }
  }
}

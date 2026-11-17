import RFB from '@novnc/novnc/lib/rfb.js';
import { Attempt, ConsoleSession } from './consoleSession';
import type { ConsoleShare } from './consoleView';
import { ConsoleSpeed, DEFAULT_SPEED, VNC_LEVELS } from './consoleSpeed';
import { consoleWsUrl, shareWsUrl } from './vncApi';

/**
 * One VNC console connection over NabiMan's proxy, apart from the view that
 * shows it. noVNC (RFB) draws into `screen`, which a view puts on the page
 * while it is up. noVNC takes keys only while its canvas has the focus, and a
 * screen that is off the page cannot have it.
 */
export class VncSession extends ConsoleSession {
  /** Where noVNC draws. */
  readonly screen = document.createElement('div');
  rfb: RFB | null = null;
  /** Text last copied inside the VM (if the guest supports it). */
  vmClip = '';
  private scale = true;
  private speed: ConsoleSpeed = DEFAULT_SPEED;
  private abort: AbortController | null = null;

  constructor(readonly vm: string, readonly share?: ConsoleShare) {
    super();
    this.screen.className = 'rw-console-kept';
  }

  /** Fit the VM's screen to the view, or ask the VM to take the view's size. */
  setScale(scale: boolean) { this.scale = scale; this.applyView(); }

  /** Picture quality against speed; noVNC applies it to the live connection. */
  setSpeed(speed: ConsoleSpeed) { this.speed = speed; this.applyView(); }

  private applyView() {
    const rfb = this.rfb;
    if (!rfb) return;
    rfb.scaleViewport = this.scale;
    // Off the page the screen measures 0×0: the VM must not be asked to match that.
    rfb.resizeSession = this.attached && !this.scale;
    rfb.qualityLevel = VNC_LEVELS[this.speed].quality;
    rfb.compressionLevel = VNC_LEVELS[this.speed].compression;
  }

  /** A view puts the screen on the page, in `host`. */
  attach(host: HTMLElement) {
    host.appendChild(this.screen);
    this.attached = true;
    this.applyView();
  }

  /** The view lets go; the connection stays. Held keys are released first. */
  detach() {
    this.attached = false;
    this.applyView();
    try { this.rfb?.blur(); } catch { /* not connected */ }
    // noVNC's own pointer (touch devices) hides when the mouse leaves the screen.
    this.screen.querySelector('canvas')?.dispatchEvent(new MouseEvent('mouseleave'));
    this.screen.remove();
  }

  protected open({ live, fail }: Attempt) {
    const { share } = this;
    const abort = new AbortController();
    this.abort = abort;
    (async () => {
      try {
        const url = share ? shareWsUrl(share.token, share.pw) : await consoleWsUrl(this.vm, abort.signal);
        if (!live()) return;
        const rfb = new RFB(this.screen, url, { wsProtocols: ['binary'] });
        rfb.viewOnly = Boolean(share?.viewOnly);
        rfb.background = '#0b0d10';
        rfb.addEventListener('connect', () => { if (live()) this.up(); });
        rfb.addEventListener('disconnect', (e: Event) => {
          if (!live()) return;
          if ((e as CustomEvent<{ clean: boolean }>).detail?.clean) this.ended();
          else fail();
        });
        rfb.addEventListener('securityfailure', () => fail({ message: 'securityfailure' }));
        rfb.addEventListener('clipboard', (e: Event) => {
          if (live()) { this.vmClip = (e as CustomEvent<{ text: string }>).detail?.text ?? ''; this.emit(); }
        });
        this.rfb = rfb;
        this.applyView();
      } catch (cause) {
        if (!abort.signal.aborted) fail({ message: cause instanceof Error ? cause.message : undefined });
      }
    })();
  }

  protected shut() {
    this.abort?.abort();
    this.abort = null;
    const rfb = this.rfb;
    this.rfb = null;
    try { rfb?.disconnect(); } catch { /* already gone */ }
  }
}

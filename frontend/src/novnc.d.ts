// noVNC ships no type declarations; this covers the one entry point we use.
declare module '@novnc/novnc/lib/rfb.js' {
  export default class RFB extends EventTarget {
    constructor(target: HTMLElement, url: string, options?: {
      shared?: boolean;
      credentials?: { username?: string; password?: string; target?: string };
      repeaterID?: string;
      wsProtocols?: string[];
    });
    scaleViewport: boolean;
    resizeSession: boolean;
    background: string;
    viewOnly: boolean;
    focusOnClick: boolean;
    disconnect(): void;
    sendCtrlAltDel(): void;
    sendKey(keysym: number, code?: string | null, down?: boolean): void;
    clipboardPasteFrom(text: string): void;
    focus(): void;
  }
}

import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import 'xterm/css/xterm.css';
import { TerminalSettings } from './settings';

const THEME = {
  background: '#0f1923', foreground: '#e0e6ed', cursor: '#3b82f6', cursorAccent: '#0f1923',
  selectionBackground: '#3b82f655', selectionForeground: '#ffffff', selectionInactiveBackground: '#3b82f633',
  black: '#1a2634', red: '#e74c3c', green: '#2ecc71', yellow: '#f39c12', blue: '#3b82f6',
  magenta: '#9b59b6', cyan: '#1abc9c', white: '#e0e6ed',
  brightBlack: '#4a5a6a', brightRed: '#ff6b6b', brightGreen: '#6bff8d', brightYellow: '#ffd93d',
  brightBlue: '#6bb3ff', brightMagenta: '#c471ed', brightCyan: '#45e6c6', brightWhite: '#ffffff',
};

/** A terminal and the addon that keeps it sized to its container. */
export function createTerminal(settings: TerminalSettings): { term: Terminal; fit: FitAddon } {
  const term = new Terminal({
    cursorBlink: true, cursorStyle: 'block', fontSize: settings.font_size,
    fontFamily: "'Fira Code', 'Cascadia Code', 'JetBrains Mono', 'Menlo', monospace",
    fontWeight: '400', fontWeightBold: '700', lineHeight: 1.15, theme: THEME,
    allowProposedApi: true, scrollback: settings.scrollback_lines,
    rightClickSelectsWord: true, convertEol: true,
  });
  const fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new WebLinksAddon());
  return { term, fit };
}

/**
 * Clipboard behaviour, in the shape terminal users expect:
 *
 * - selecting with the mouse copies straight away (drag, double-click a word,
 *   triple-click a line),
 * - right-click pastes,
 * - Ctrl+Shift+C / Ctrl+Shift+V still work.
 *
 * The selection usually belongs to tmux rather than to xterm — tmux runs with
 * mouse mode on so the wheel scrolls its history — so tmux is configured to
 * hand the copied text over as an OSC 52 sequence, which is what the handler
 * below turns into a clipboard write. The xterm-side selection is copied too,
 * for panes where tmux is not in the way (a Shift-drag, for instance).
 */
export function attachClipboard(
  term: Terminal,
  container: HTMLElement | null,
  send: (data: string) => void,
): () => void {
  const copy = (text: string) => {
    if (text) navigator.clipboard.writeText(text).catch(() => {});
  };
  const paste = () => {
    navigator.clipboard.readText().then(send).catch(() => {});
  };

  // tmux (and any other full-screen app) asks the terminal to set the clipboard
  // with OSC 52; xterm.js leaves that to the embedder for safety.
  const oscDispose = term.parser.registerOscHandler(52, (data: string) => {
    const payload = data.slice(data.indexOf(';') + 1);
    try {
      const binary = atob(payload);
      const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
      copy(new TextDecoder().decode(bytes));
    } catch { /* not base64: nothing to copy */ }
    return true;
  });

  const copyLocalSelection = () => {
    if (term.hasSelection()) copy(term.getSelection());
  };

  term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
    if (ev.type !== 'keydown') return true;
    if (ev.ctrlKey && ev.shiftKey && ev.key === 'C') { copyLocalSelection(); return false; }
    if (ev.ctrlKey && ev.shiftKey && ev.key === 'V') { paste(); return false; }
    return true;
  });

  const onContextMenu = (ev: MouseEvent) => {
    ev.preventDefault();
    paste();
  };
  const onMouseUp = () => copyLocalSelection();

  container?.addEventListener('contextmenu', onContextMenu);
  container?.addEventListener('mouseup', onMouseUp);
  container?.addEventListener('dblclick', onMouseUp);

  return () => {
    oscDispose.dispose();
    container?.removeEventListener('contextmenu', onContextMenu);
    container?.removeEventListener('mouseup', onMouseUp);
    container?.removeEventListener('dblclick', onMouseUp);
  };
}

/**
 * Wire the terminal's output back to whoever is carrying it: keystrokes,
 * pasted bytes, and (for panes that own their session) resizes.
 */
export function attachTerminalIo(
  term: Terminal,
  send: (data: string | ArrayBuffer) => void,
  sendResize: ((cols: number, rows: number) => void) | null,
): void {
  term.onData((data) => send(data));
  term.onBinary((data) => {
    const buf = new Uint8Array(data.length);
    for (let i = 0; i < data.length; i++) buf[i] = data.charCodeAt(i);
    send(buf.buffer);
  });
  term.onResize(({ cols, rows }) => sendResize?.(cols, rows));
}

/**
 * Fit a grid of `cols x rows` into the container by changing the font size
 * rather than the number of cells — what a share viewer needs, since resizing
 * would reshape the terminal its owner is working in.
 *
 * The measurement comes from the fit addon, which knows what a cell actually
 * costs at the current font; guessing from the font size alone is off by
 * enough to clip a line.
 */
export function fitFontToPane(term: Terminal, fit: FitAddon, cols: number, rows: number): void {
  if (!cols || !rows) return;
  const fits = () => {
    const proposed = fit.proposeDimensions();
    return Boolean(proposed && proposed.cols >= cols && proposed.rows >= rows);
  };

  let size = term.options.fontSize || 14;
  if (fits()) {
    // Room to spare: grow until one more step would clip.
    while (size < 28) {
      term.options.fontSize = size + 0.5;
      if (!fits()) { term.options.fontSize = size; return; }
      size += 0.5;
    }
    return;
  }
  while (size > 6) {
    size -= 0.5;
    term.options.fontSize = size;
    if (fits()) return;
  }
}

/**
 * Ctrl + wheel zooms the text. A plain wheel is left alone so xterm forwards it
 * to tmux, which scrolls its own history like a native terminal.
 */
export function attachWheelZoom(
  container: HTMLElement,
  getTerm: () => Terminal | null,
  fit: () => void,
  onZoom?: (size: number) => void,
): () => void {
  const onWheel = (e: WheelEvent) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    e.stopPropagation();
    const term = getTerm();
    if (!term) return;
    const size = Math.min(28, Math.max(8, (term.options.fontSize || 14) + (e.deltaY < 0 ? 1 : -1)));
    term.options.fontSize = size;
    onZoom?.(size);
    fit();
  };
  container.addEventListener('wheel', onWheel, { passive: false, capture: true });
  return () => container.removeEventListener('wheel', onWheel, { capture: true } as EventListenerOptions);
}

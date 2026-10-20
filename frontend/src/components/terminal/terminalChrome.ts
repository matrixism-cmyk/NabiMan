import { Terminal } from 'xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import 'xterm/css/xterm.css';
import './terminal-symbols.css';
import { TerminalSettings } from './settings';
import { applyArmed } from './stickyKeys';
import { blobsFromClipboardItems, pathsForClipboardFiles } from './pasteFile';

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
    fontFamily: "'NabiMan Circled Digits', 'Fira Code', 'Cascadia Code', 'JetBrains Mono', 'Menlo', monospace",
    fontWeight: '400', fontWeightBold: '700', lineHeight: 1.15, theme: THEME,
    allowProposedApi: true, scrollback: settings.scrollback_lines,
    // A real PTY supplies CR/LF itself. Converting LF to CRLF corrupts
    // tmux's cursor-relative redraws (notably while scrolling copy-mode).
    rightClickSelectsWord: true, convertEol: false,
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
 * - Ctrl+V, Ctrl+Shift+V, right-click and middle-click all paste, once each,
 * - and every paste is handed to the server as one piece, which passes it to
 *   tmux. tmux leaves copy-mode first and brackets the paste, so pasting works
 *   with a selection on screen and an editor can tell it from typing.
 *
 * The selection itself usually belongs to tmux rather than to xterm — tmux runs
 * with mouse mode on so the wheel scrolls its history — so tmux hands the
 * copied text over as an OSC 52 sequence, which the handler below turns into a
 * clipboard write. The xterm-side selection is copied too, for panes where tmux
 * is not in the way.
 */
export function attachClipboard(
  term: Terminal,
  container: HTMLElement | null,
  sendPaste: (text: string) => void,
): () => void {
  const copy = (text: string) => {
    if (text) navigator.clipboard.writeText(text).catch(() => {});
  };
  // Files on the clipboard (a screenshot, say) are uploaded and pasted as the
  // path the server saved them at; see ./pasteFile.
  const pasteFiles = (files: Array<File | Blob>) => {
    pathsForClipboardFiles(files).then((paths) => { if (paths) sendPaste(paths); }).catch(() => {});
  };
  const pasteFromClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) { sendPaste(text); return; }
    } catch { /* no text, or no permission: try the file side below */ }
    if (!navigator.clipboard.read) return;
    try {
      pasteFiles(await blobsFromClipboardItems(await navigator.clipboard.read()));
    } catch { /* nothing usable on the clipboard */ }
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

  // Ctrl+Shift+V is also the browser's own "paste as plain text", so it usually
  // arrives as a paste event by itself — pasting from the key handler as well
  // put the clipboard in twice. So the key handler waits a moment: if no paste
  // event turns up (some browsers keep the shortcut to themselves), it reads
  // the clipboard instead.
  let lastPasteEvent = 0;
  let shortcutTimer: ReturnType<typeof setTimeout> | null = null;
  const clearShortcutTimer = () => {
    if (shortcutTimer) { clearTimeout(shortcutTimer); shortcutTimer = null; }
  };
  const onShortcutPaste = () => {
    clearShortcutTimer();
    shortcutTimer = setTimeout(() => {
      shortcutTimer = null;
      if (Date.now() - lastPasteEvent < 400) return;
      pasteFromClipboard();
    }, 250);
  };

  term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
    if (ev.type !== 'keydown') return true;
    if (ev.ctrlKey && ev.shiftKey && ev.key === 'C') {
      ev.preventDefault();
      copyLocalSelection();
      return false;
    }
    if (ev.ctrlKey && ev.shiftKey && ev.key === 'V') { onShortcutPaste(); return false; }
    return true;
  });

  // Ctrl+V / Cmd+V arrive as a paste event on xterm's textarea. Taking it here
  // keeps every paste on the same path as the mouse ones.
  const onPaste = (ev: ClipboardEvent) => {
    lastPasteEvent = Date.now();
    clearShortcutTimer();
    const files = Array.from(ev.clipboardData?.files || []);
    if (files.length) {
      ev.preventDefault();
      ev.stopPropagation();
      pasteFiles(files);
      return;
    }
    const text = ev.clipboardData?.getData('text/plain');
    if (!text) return;
    ev.preventDefault();
    ev.stopPropagation();
    sendPaste(text);
  };
  const onContextMenu = (ev: MouseEvent) => {
    ev.preventDefault();
    pasteFromClipboard();
  };
  // Middle-click is the X11 habit; tmux does nothing with it, so it is ours.
  const onMouseDown = (ev: MouseEvent) => {
    if (ev.button !== 1) return;
    ev.preventDefault();
    pasteFromClipboard();
  };
  const onMouseUp = () => copyLocalSelection();

  container?.addEventListener('paste', onPaste, true);
  container?.addEventListener('contextmenu', onContextMenu);
  container?.addEventListener('mousedown', onMouseDown);
  container?.addEventListener('mouseup', onMouseUp);
  container?.addEventListener('dblclick', onMouseUp);

  return () => {
    clearShortcutTimer();
    oscDispose.dispose();
    container?.removeEventListener('paste', onPaste, true);
    container?.removeEventListener('contextmenu', onContextMenu);
    container?.removeEventListener('mousedown', onMouseDown);
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
  // Ctrl or Alt tapped on a phone's key bar applies to the next key typed.
  term.onData((data) => send(applyArmed(term, data)));
  term.onBinary((data) => {
    const buf = new Uint8Array(data.length);
    for (let i = 0; i < data.length; i++) buf[i] = data.charCodeAt(i);
    send(buf.buffer);
  });
  term.onResize(({ cols, rows }) => sendResize?.(cols, rows));
}

/**
 * Fill the container with a grid of `cols x rows` without changing the grid.
 *
 * The font size is chosen so the columns span the width, and the line height
 * then stretches the rows down the remaining height. Stretching the spacing
 * rather than the glyphs keeps the text undistorted while leaving no dead
 * margin — a shared pane is usually a different shape from the window it is
 * being watched in.
 */
export function fitFontToPane(
  term: Terminal,
  fit: FitAddon,
  cols: number,
  rows: number,
  maxSize = 28,
): void {
  if (!cols || !rows) return;
  const proposal = () => fit.proposeDimensions();
  const widthFits = () => { const p = proposal(); return Boolean(p && p.cols >= cols); };
  const heightFits = () => { const p = proposal(); return Boolean(p && p.rows >= rows); };

  // 1. The widest text that still shows every column.
  term.options.lineHeight = 1;
  let size = term.options.fontSize || 14;
  if (widthFits()) {
    while (size < maxSize) {
      term.options.fontSize = size + 0.5;
      if (!widthFits()) { term.options.fontSize = size; break; }
      size += 0.5;
    }
  } else {
    while (size > 6) {
      size -= 0.5;
      term.options.fontSize = size;
      if (widthFits()) break;
    }
  }

  // 2. Spread the rows over whatever height is left.
  let lineHeight = 1;
  if (!heightFits()) {
    // Too tall even packed tight: give the height priority and shrink the text.
    while (size > 6 && !heightFits()) {
      size -= 0.5;
      term.options.fontSize = size;
    }
  } else {
    while (lineHeight < 2.4) {
      term.options.lineHeight = lineHeight + 0.05;
      if (!heightFits()) { term.options.lineHeight = lineHeight; break; }
      lineHeight += 0.05;
    }
  }
}

/**
 * Take on another terminal's grid, leaving the text size alone.
 *
 * A watched pane has to keep the owner's grid — tmux paints one screen for it,
 * so a different number of columns would simply be wrong — but how big that
 * grid is drawn is the viewer's own business.
 */
export function adoptGrid(term: Terminal, cols: number, rows: number): void {
  if (cols <= 0 || rows <= 0) return;
  if (term.cols === cols && term.rows === rows) return;
  term.resize(cols, rows);
}

/**
 * Take on another terminal's grid and scale the text to fill the window. Used
 * until the viewer zooms by hand, after which the text size is theirs to keep.
 * A mirrored pane may grow past the normal zoom cap: filling the window is the
 * point.
 */
export function mirrorGrid(term: Terminal, fit: FitAddon, cols: number, rows: number): void {
  if (cols <= 0 || rows <= 0) return;
  adoptGrid(term, cols, rows);
  fitFontToPane(term, fit, cols, rows, 44);
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

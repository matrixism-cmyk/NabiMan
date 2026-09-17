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
 * Copy and paste the way a terminal user expects: Ctrl+Shift+C/V, and a
 * right-click that copies a selection or pastes when there is none — the
 * habits PuTTY taught. Returns a cleanup function.
 */
export function attachClipboard(
  term: Terminal,
  container: HTMLElement | null,
  send: (data: string) => void,
): () => void {
  const paste = () => {
    navigator.clipboard.readText().then(send).catch(() => {});
  };

  term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
    if (ev.type !== 'keydown') return true;
    if (ev.ctrlKey && ev.shiftKey && ev.key === 'C') {
      const sel = term.getSelection();
      if (sel) navigator.clipboard.writeText(sel).catch(() => {});
      return false;
    }
    if (ev.ctrlKey && ev.shiftKey && ev.key === 'V') { paste(); return false; }
    return true;
  });

  const onContextMenu = (ev: MouseEvent) => {
    ev.preventDefault();
    const sel = term.getSelection();
    if (sel) {
      navigator.clipboard.writeText(sel).catch(() => {});
      term.clearSelection();
    } else {
      paste();
    }
  };
  container?.addEventListener('contextmenu', onContextMenu);
  return () => container?.removeEventListener('contextmenu', onContextMenu);
}

/**
 * Ctrl + wheel zooms the text. A plain wheel is left alone so xterm forwards it
 * to tmux, which scrolls its own history like a native terminal.
 */
export function attachWheelZoom(
  container: HTMLElement,
  getTerm: () => Terminal | null,
  fit: () => void,
): () => void {
  const onWheel = (e: WheelEvent) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    e.stopPropagation();
    const term = getTerm();
    if (!term) return;
    term.options.fontSize = Math.min(28, Math.max(8, (term.options.fontSize || 14) + (e.deltaY < 0 ? 1 : -1)));
    fit();
  };
  container.addEventListener('wheel', onWheel, { passive: false, capture: true });
  return () => container.removeEventListener('wheel', onWheel, { capture: true } as EventListenerOptions);
}

import type { IDisposable, ITerminalAddon, Terminal } from 'xterm';
import { WebglAddon } from 'xterm-addon-webgl';
import { CanvasAddon } from 'xterm-addon-canvas';

/**
 * How a terminal draws: WebGL first, then Canvas, then xterm's own DOM.
 *
 * The DOM renderer draws box-drawing characters with the font, so with
 * lineHeight 1.15 vertical lines break between rows, and a cell's own
 * background (an inline style) hides the block cursor. Canvas and WebGL draw
 * those characters to fill the cell and the cursor on its own layer.
 *
 * WebGL is the fastest but not always there: a browser allows only so many
 * WebGL contexts per page (each terminal window holds one), some machines have
 * no GPU acceleration (remote desktops, VMs) and get a slow software WebGL, and
 * a GPU reset loses the context mid-session. Each of those falls back to Canvas
 * for that window only, and Canvas to DOM.
 *
 * The choice is per browser (localStorage), since what works depends on the
 * device: 'auto' or one renderer to start from (later ones remain fallbacks).
 */
export type RendererChoice = 'auto' | 'webgl' | 'canvas' | 'dom';
export type Renderer = 'webgl' | 'canvas' | 'dom';
/** Why a terminal is not on the first renderer it tried. */
export type FallbackReason = 'software-webgl' | 'webgl-failed' | 'webgl-lost' | 'canvas-failed';

export interface RendererState { active: Renderer; reason?: FallbackReason }

const CHOICE_KEY = 'nabiman.terminalRenderer';
const CHOICES: RendererChoice[] = ['auto', 'webgl', 'canvas', 'dom'];

export function readRendererChoice(): RendererChoice {
  try {
    const v = localStorage.getItem(CHOICE_KEY) as RendererChoice | null;
    return v && CHOICES.includes(v) ? v : 'auto';
  } catch {
    return 'auto';
  }
}

export function writeRendererChoice(choice: RendererChoice): void {
  try {
    if (choice === 'auto') localStorage.removeItem(CHOICE_KEY);
    else localStorage.setItem(CHOICE_KEY, choice);
  } catch { /* private mode: stays 'auto' */ }
}

/** WebGL done on the CPU (Chrome's SwiftShader, Mesa's llvmpipe) is slower than Canvas. */
export function isSoftwareWebgl(doc: Document = document): boolean {
  try {
    const canvas = doc.createElement('canvas');
    const gl = canvas.getContext('webgl2') as WebGL2RenderingContext | null;
    if (!gl) return false; // no WebGL at all: loading the addon fails and says so
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    const name = String(info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER));
    gl.getExtension('WEBGL_lose_context')?.loseContext(); // give the context back at once
    return /swiftshader|llvmpipe|softpipe|software/i.test(name);
  } catch {
    return false;
  }
}

/** Live terminals and how each draws (for the settings dialog). */
const states = new Map<Terminal, RendererState>();

export function rendererStates(): RendererState[] {
  return Array.from(states.values());
}

export function rendererState(term: Terminal): RendererState | undefined {
  return states.get(term);
}

export interface RendererAddons {
  webgl: () => ITerminalAddon & { onContextLoss?: (cb: () => void) => IDisposable };
  canvas: () => ITerminalAddon;
  softwareWebgl: () => boolean;
}

const REAL: RendererAddons = {
  webgl: () => new WebglAddon(),
  canvas: () => new CanvasAddon(),
  softwareWebgl: () => isSoftwareWebgl(),
};

/**
 * Picks the renderer for an opened terminal (`term.open` first: the addons
 * need its element). Dispose before the terminal.
 */
export function attachRenderer(term: Terminal, choice: RendererChoice, addons: RendererAddons = REAL): IDisposable {
  let current: IDisposable | null = null;
  let lossWatch: IDisposable | null = null;
  const set = (active: Renderer, reason?: FallbackReason) => states.set(term, reason ? { active, reason } : { active });

  const startCanvas = (reason?: FallbackReason) => {
    try {
      const addon = addons.canvas();
      term.loadAddon(addon);
      current = addon;
      set('canvas', reason);
    } catch {
      current = null;
      set('dom', 'canvas-failed');
    }
  };

  const startWebgl = (): boolean => {
    try {
      const addon = addons.webgl();
      term.loadAddon(addon);
      current = addon;
      // Lost later (GPU reset, too many contexts): this window carries on with Canvas
      lossWatch = addon.onContextLoss?.(() => {
        lossWatch?.dispose();
        addon.dispose();
        startCanvas('webgl-lost');
      }) ?? null;
      set('webgl');
      return true;
    } catch {
      return false;
    }
  };

  if (choice === 'dom') {
    set('dom');
  } else if (choice === 'canvas') {
    startCanvas();
  } else if (choice === 'auto' && addons.softwareWebgl()) {
    startCanvas('software-webgl');
  } else if (!startWebgl()) {
    startCanvas('webgl-failed');
  }

  return {
    dispose: () => {
      lossWatch?.dispose();
      current?.dispose();
      states.delete(term);
    },
  };
}

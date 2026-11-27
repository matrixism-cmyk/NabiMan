/** Geometry shared by the window manager and the window frame. */

import { TerminalTarget } from './TerminalView';

/** The VM console a console window shows. */
export interface ConsoleRef {
  /** The console's key: a built-in bridge's VM name, or a registered one's id. */
  name: string;
  label: string;
  protocol: 'vnc' | 'rdp';
  builtin: boolean;
}

interface WindowBase {
  id: string;
  title: string;
  subtitle: string;
  x: number;
  y: number;
  w: number;
  h: number;
  z: number;
  minimized: boolean;
  maximized: boolean;
  /** Geometry to return to when un-maximizing. */
  restore?: { x: number; y: number; w: number; h: number };
}

/** A window on a tmux-backed terminal session. */
export interface TerminalWin extends WindowBase {
  kind: 'terminal';
  /** The session's own name, once it has one: the taskbar shows it after the title. */
  sessionName?: string;
  target: TerminalTarget;
}

/** A window on a VM console (RDP or VNC). */
export interface ConsoleWin extends WindowBase {
  kind: 'console';
  console: ConsoleRef;
}

/** A floating window: a terminal, or a VM console. Layouts saved before
 *  consoles had windows carry no `kind`; they load as terminals (windowStore). */
export type TermWindow = TerminalWin | ConsoleWin;

/** What a frame may change about its window. */
export type WindowPatch = Partial<Omit<WindowBase, 'id'>> & { sessionName?: string };

export const DEFAULT_W = 720;
export const DEFAULT_H = 440;
/** A desktop wants more room than a shell. */
export const CONSOLE_W = 1040;
export const CONSOLE_H = 720;
export const MIN_W = 320;
export const MIN_H = 200;
export const TASKBAR_H = 44;

export const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

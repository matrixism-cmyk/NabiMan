/** Geometry shared by the window manager and the window frame. */

import { TerminalTarget } from './TerminalView';

export interface TermWindow {
  id: string;
  title: string;
  subtitle: string;
  target: TerminalTarget;
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

export const DEFAULT_W = 720;
export const DEFAULT_H = 440;
export const MIN_W = 320;
export const MIN_H = 200;
export const TASKBAR_H = 44;

export const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

/**
 * The colours the map is drawn in, by style and by the app's theme (light
 * themes get a light stage with dark grey links in the simple style), and
 * the two things read from the page: the theme's mode and reduced motion.
 */
import { useEffect, useState } from 'react';
import type { GraphStyle } from './graphSettings';

export interface Palette {
  /** Links at rest: a neutral grey. */
  link: string;
  linkOpacity: number;
  /** Host, connection and network threads: quieter still. */
  threadOpacity: number;
  /** A faded link while something else is in focus. */
  fadedOpacity: number;
  /** A dimmed node while something else is in focus. */
  dimmed: number;
  label: string;
  labelBox: string;
  sub: string;
  /** Group colours off: every node this colour. */
  node: string;
  ended: string;
  lead: string;
  busy: string;
  stars: number;
}

const SIMPLE_DARK: Palette = {
  link: '#8b949e', linkOpacity: 0.26, threadOpacity: 0.16, fadedOpacity: 0.04, dimmed: 0.14,
  label: '#d0d7de', labelBox: '', sub: '#8b949e', node: '#a5adb8', ended: '#4b5563', lead: '#fbbf24', busy: '#4ade80', stars: 0.22,
};
const SIMPLE_LIGHT: Palette = {
  link: '#57606a', linkOpacity: 0.34, threadOpacity: 0.22, fadedOpacity: 0.06, dimmed: 0.18,
  label: '#24292f', labelBox: '', sub: '#57606a', node: '#6e7781', ended: '#c4c9d0', lead: '#d97706', busy: '#16a34a', stars: 0,
};
const RICH: Palette = {
  link: '#64748b', linkOpacity: 0.85, threadOpacity: 0.85, fadedOpacity: 0.08, dimmed: 0.16,
  label: '#e6edf3', labelBox: 'rgba(11,18,32,0.6)', sub: '#9da7b3', node: '#94a3b8', ended: '#64748b', lead: '#fbbf24', busy: '#4ade80', stars: 0.55,
};

/** The rich style is a night sky whatever the theme; the simple one follows it. */
export const paletteOf = (style: GraphStyle, light: boolean): Palette => (style === 'rich' ? RICH : light ? SIMPLE_LIGHT : SIMPLE_DARK);

function readLight(): boolean {
  if (typeof document === 'undefined') return false;
  const mode = document.documentElement.getAttribute('data-theme-mode');
  if (mode) return mode === 'light';
  try { return window.matchMedia?.('(prefers-color-scheme: light)').matches ?? false; } catch { return false; }
}

/** Whether the app is in a light theme now (ThemeProvider sets `data-theme-mode` on <html>), following changes. */
export function useLightTheme(): boolean {
  const [light, setLight] = useState(readLight);
  useEffect(() => {
    const watch = new MutationObserver(() => setLight(readLight()));
    watch.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme-mode'] });
    return () => watch.disconnect();
  }, []);
  return light;
}

function readReduced(): boolean {
  try { return typeof window !== 'undefined' && !!window.matchMedia?.('(prefers-reduced-motion: reduce)').matches; } catch { return false; }
}

/** The visitor asked for less motion: no turning, no breathing. */
export function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(readReduced);
  useEffect(() => {
    let q: MediaQueryList | undefined;
    try { q = window.matchMedia?.('(prefers-reduced-motion: reduce)'); } catch { q = undefined; }
    if (!q) return undefined;
    const on = () => setReduced(q!.matches);
    q.addEventListener?.('change', on);
    return () => q!.removeEventListener?.('change', on);
  }, []);
  return reduced;
}

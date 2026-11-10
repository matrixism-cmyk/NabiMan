import { MutableRefObject, RefObject, useEffect } from 'react';
import { Terminal } from 'xterm';
import { applyTerminalFont } from './terminalFonts';
import type { TerminalSettings } from './settings';
import type { TerminalZoom } from './useTerminalZoom';

interface Options {
  containerRef: RefObject<HTMLDivElement | null>;
  termRef: MutableRefObject<Terminal | null>;
  settings: TerminalSettings;
  active?: boolean;
  zoom: TerminalZoom;
  /** The settings size the pane last applied, so a zoom is not undone by it. */
  appliedFontRef: MutableRefObject<number>;
  appliedFamilyRef: MutableRefObject<TerminalSettings['font_family']>;
}

/**
 * Keeps a mounted pane laid out: sized to its container, in step with the
 * settings (scrollback, font, size), and focused when it becomes active.
 */
export function useTerminalLayout({
  containerRef, termRef, settings, active, zoom, appliedFontRef, appliedFamilyRef,
}: Options) {
  // Keep the screen sized to its container (window drag/resize, layout change).
  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    let frame = 0;
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(zoom.relayout);
    });
    observer.observe(el);
    return () => { cancelAnimationFrame(frame); observer.disconnect(); };
  }, [containerRef, zoom.relayout]);

  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    term.options.scrollback = settings.scrollback_lines;
    if (settings.font_family !== appliedFamilyRef.current) {
      appliedFamilyRef.current = settings.font_family;
      applyTerminalFont(term, settings.font_family, zoom.relayout, () => termRef.current === term);
    }
    // Changing the size in settings is deliberate, so it clears a pane's zoom.
    if (settings.font_size !== appliedFontRef.current) {
      appliedFontRef.current = settings.font_size;
      zoom.forget(settings.font_size);
      return;
    }
    zoom.relayout();
  }, [appliedFamilyRef, appliedFontRef, settings.font_family, settings.font_size, settings.scrollback_lines, termRef, zoom]);

  useEffect(() => {
    if (!active) return;
    const id = window.setTimeout(() => {
      zoom.relayout();
      termRef.current?.focus();
    }, 50);
    return () => window.clearTimeout(id);
  }, [active, termRef, zoom]);
}

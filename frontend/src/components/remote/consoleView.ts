import { useCallback, useState } from 'react';

/**
 * A VM console's own view settings, kept in this browser (like the terminal
 * share view). Unset fields follow the account defaults. Dictation is what a
 * graphical console can meaningfully tune per session.
 */
export interface ConsoleViewPrefs {
  dictation_pause_ms?: number;
  dictation_auto_enter?: boolean;
  /** Only the enrolled voice (a signed-in account's own; never a share visitor's). */
  dictation_voice_lock?: boolean;
  /** RDP: `fit` follows the window; `WxH` keeps a fixed desktop, scaled to fit. */
  resolution?: string;
  /** RDP on a touch screen: tap where you touch, or move the pointer like a touchpad. */
  touch?: 'screen' | 'pad';
  /** Keep the view connected: reconnect automatically on an unexpected drop
   *  (default on). The VM's own desktop session persists regardless. */
  auto_reconnect?: boolean;
}

/** What a console share link lets its visitor do; dictation is null when they cannot dictate. */
export interface ConsoleShare {
  token: string;
  viewOnly: boolean;
  pw?: string;
  dictation?: { pause_ms: number; auto_enter: boolean } | null;
}

/** Fixed desktop sizes offered besides following the window. */
export const RESOLUTIONS = ['1280x720', '1600x900', '1920x1080'];

/** The fixed desktop size, or null to follow the window. */
export function fixedSize(view: ConsoleViewPrefs): { w: number; h: number } | null {
  const m = /^(\d{3,4})x(\d{3,4})$/.exec(view.resolution || '');
  return m && RESOLUTIONS.includes(view.resolution!) ? { w: Number(m[1]), h: Number(m[2]) } : null;
}

/** Whether the view should auto-reconnect (the default when unset). */
export const keepAlive = (view: ConsoleViewPrefs): boolean => view.auto_reconnect !== false;

const KEY = 'nabiman_console_view';

function read(): ConsoleViewPrefs {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) || '{}');
    return value && typeof value === 'object' ? value : {};
  } catch { return {}; }
}

export function useConsoleView(): [ConsoleViewPrefs, (next: ConsoleViewPrefs) => void] {
  const [view, setView] = useState<ConsoleViewPrefs>(read);
  const save = useCallback((next: ConsoleViewPrefs) => {
    setView(next);
    try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* private mode: this visit only */ }
  }, []);
  return [view, save];
}

/** The dictation props a console passes to AccountDictation, overriding the
 *  account defaults only where this browser set them. */
export function consoleDictationProps(view: ConsoleViewPrefs): { endpointMs?: number; enterOnPause?: boolean; voiceLock?: boolean } {
  const props: { endpointMs?: number; enterOnPause?: boolean; voiceLock?: boolean } = {};
  if (view.dictation_pause_ms != null) props.endpointMs = view.dictation_pause_ms;
  if (view.dictation_auto_enter != null) props.enterOnPause = view.dictation_auto_enter;
  if (view.dictation_voice_lock != null) props.voiceLock = view.dictation_voice_lock;
  return props;
}

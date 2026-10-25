import { useCallback, useState } from 'react';

/**
 * A VM console's own view settings, kept in this browser (like the terminal
 * share view). Unset fields follow the account defaults. Dictation is what a
 * graphical console can meaningfully tune per session.
 */
export interface ConsoleViewPrefs {
  dictation_pause_ms?: number;
  dictation_auto_enter?: boolean;
}

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
export function consoleDictationProps(view: ConsoleViewPrefs): { endpointMs?: number; enterOnPause?: boolean } {
  const props: { endpointMs?: number; enterOnPause?: boolean } = {};
  if (view.dictation_pause_ms != null) props.endpointMs = view.dictation_pause_ms;
  if (view.dictation_auto_enter != null) props.enterOnPause = view.dictation_auto_enter;
  return props;
}

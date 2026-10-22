import React, { useCallback, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import { DEFAULT_TERMINAL_SETTINGS, TerminalSettings } from './settings';
import { fontOf, TERMINAL_FONTS } from './terminalFonts';
import { PAUSES } from './dictation/PauseField';

/**
 * A share link's visitor's own view: kept in their browser, never sent
 * anywhere, so the person who shared the session sees no change. Unset, it
 * follows the defaults (and the server's dictation settings). The text
 * size is not here: a visitor's view is sized to the shared screen's grid,
 * and Ctrl+wheel changes it.
 */
export interface ShareView {
  font_family?: string;
  dictation_pause_ms?: number;
  dictation_auto_enter?: boolean;
}

const KEY = 'nabiman_share_view';

function read(): ShareView {
  try {
    const value = JSON.parse(localStorage.getItem(KEY) || '{}');
    return value && typeof value === 'object' ? value : {};
  } catch { return {}; }
}

export function useShareView(): [ShareView, (next: ShareView) => void] {
  const [view, setView] = useState<ShareView>(read);
  const save = useCallback((next: ShareView) => {
    setView(next);
    try { localStorage.setItem(KEY, JSON.stringify(next)); } catch { /* private mode: for this visit only */ }
  }, []);
  return [view, save];
}

/** The terminal settings the visitor's view draws with. */
export const viewSettings = (view: ShareView): TerminalSettings => ({
  ...DEFAULT_TERMINAL_SETTINGS,
  font_family: fontOf(view.font_family ?? DEFAULT_TERMINAL_SETTINGS.font_family).id,
});

interface Props {
  view: ShareView;
  onChange: (next: ShareView) => void;
  /** The server's dictation settings, when the link lets the visitor dictate. */
  dictation: { pause_ms: number; auto_enter: boolean } | null;
  onClose: () => void;
}

export default function ShareViewSettings({ view, onChange, dictation, onClose }: Props) {
  const { t } = useT();
  const shown = viewSettings(view);
  const seconds = (ms: number) => (ms === 0 ? t('dictation.pauseOff') : t('dictation.pauseSeconds', { seconds: String(ms / 1000) }));
  const set = (patch: ShareView) => onChange({ ...view, ...patch });
  return (
    <Modal onClose={onClose} title={t('share.viewSettings')} className="share-view-settings">
      <p className="text-secondary">{t('share.viewSettingsHint')}</p>
      <div className="form-group">
        <label htmlFor="sv-font">{t('terminal.font')}</label>
        <select id="sv-font" className="select-input" value={shown.font_family} onChange={(e) => set({ font_family: e.target.value })}>
          {TERMINAL_FONTS.map((f) => <option key={f.id} value={f.id}>{f.id === 'system' ? t('terminal.fontSystem') : f.label}</option>)}
        </select>
      </div>
      <p className="text-secondary">{t('share.viewSizeHint')}</p>
      {dictation && (<>
        <div className="form-group">
          <label htmlFor="sv-pause">{t('dictation.pause')}</label>
          <select id="sv-pause" className="select-input" value={view.dictation_pause_ms ?? dictation.pause_ms}
            onChange={(e) => set({ dictation_pause_ms: Number(e.target.value) })}>
            {(PAUSES.includes(dictation.pause_ms) ? PAUSES : [...PAUSES, dictation.pause_ms]).map((ms) => <option key={ms} value={ms}>{seconds(ms)}</option>)}
          </select>
        </div>
        <label className="session-follow">
          <input type="checkbox" checked={view.dictation_auto_enter ?? dictation.auto_enter}
            onChange={(e) => set({ dictation_auto_enter: e.target.checked })} />
          <span>{t('dictation.autoEnter')}</span>
        </label>
      </>)}
      <div className="btn-group">
        <button type="button" className="btn btn-secondary" onClick={() => onChange({})}>{t('share.viewReset')}</button>
        <button type="button" className="btn btn-primary" onClick={onClose}>{t('common.close')}</button>
      </div>
    </Modal>
  );
}

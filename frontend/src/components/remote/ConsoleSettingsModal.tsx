import React from 'react';
import Modal from '../../shared/ui/Modal';
import { useT } from '../../shared/i18n';
import PauseField from '../terminal/dictation/PauseField';
import { ConsoleViewPrefs } from './consoleView';

/**
 * A VM console's settings, like the SSH session's: this browser's own view,
 * never sent anywhere. A graphical console tunes its dictation here; the rest
 * (connect method, access, sharing) stays on the console bar.
 */
export default function ConsoleSettingsModal({ view, onChange, onClose }: {
  view: ConsoleViewPrefs;
  onChange: (next: ConsoleViewPrefs) => void;
  onClose: () => void;
}) {
  const { t } = useT();
  return (
    <Modal className="share-dialog" onClose={onClose} title={t('remote.console.settings')}>
      <div className="form-group">
        <label>
          <input type="checkbox" checked={view.auto_reconnect !== false}
            onChange={(e) => onChange({ ...view, auto_reconnect: e.target.checked })} />{' '}
          {t('remote.console.keepalive')}
        </label>
        <small className="text-secondary">{t('remote.console.keepaliveHint')}</small>
      </div>
      <div className="form-group">
        <label htmlFor="console-pause">{t('dictation.pause')}</label>
        <PauseField id="console-pause" value={view.dictation_pause_ms ?? 1500}
          onChange={(ms) => onChange({ ...view, dictation_pause_ms: ms })} />
        <small className="text-secondary">{t('dictation.pauseHint')}</small>
      </div>
      <div className="form-group">
        <label>
          <input type="checkbox" checked={Boolean(view.dictation_auto_enter)}
            onChange={(e) => onChange({ ...view, dictation_auto_enter: e.target.checked })} />{' '}
          {t('dictation.autoEnter')}
        </label>
        <small className="text-secondary">{t('dictation.autoEnterHint')}</small>
      </div>
      <div className="btn-group">
        <button className="btn btn-secondary" onClick={onClose}>{t('common.close')}</button>
      </div>
    </Modal>
  );
}

import React from 'react';
import { useT } from '../../i18n';
import TerminalSettingsForm from '../terminal/TerminalSettingsForm';
import DictationSettings from '../terminal/dictation/DictationSettings';
import { useChromeHidden } from '../../hooks/useChromeHidden';

/**
 * The remote workspace's own settings, in one place: the screen, terminals
 * (font, history, keep-alive) and dictation. Remote keeps its settings to
 * itself, so it can stand on its own one day.
 */
export default function RemoteSettingsView() {
  const { t } = useT();
  const [hidden, setHidden] = useChromeHidden();
  return (
    <div className="rw-settings">
      <section className="rw-settings-section">
        <h3>{t('remote.settings.screen')}</h3>
        <label className="rw-settings-check">
          <input type="checkbox" checked={hidden} onChange={(e) => setHidden(e.target.checked)} />
          <span>{t('remote.settings.hideChrome')}</span>
        </label>
        <small className="text-secondary">{t('remote.settings.hideChromeHint')}</small>
      </section>
      <section className="rw-settings-section">
        <DictationSettings />
      </section>
      <section className="rw-settings-section is-wide">
        <h3>{t('terminal.settings')}</h3>
        <TerminalSettingsForm />
      </section>
    </div>
  );
}

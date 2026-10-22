import React from 'react';
import { useT } from '../../shared/i18n';
import TerminalSettingsForm from '../terminal/TerminalSettingsForm';
import DictationSettings from '../terminal/dictation/DictationSettings';
import { useMember } from './member';

/**
 * The remote workspace's own settings, in one place: dictation, and
 * terminals (font, history, keep-alive). Remote keeps its settings to
 * itself, so it can stand on its own one day. (Folding NabiMan's top away
 * is the small tab under its menu.)
 */
export default function RemoteSettingsView() {
  const { t } = useT();
  // Dictation's settings are the whole server's; a member just uses them.
  const member = useMember();
  return (
    <div className="rw-settings">
      {!member && (
        <section className="rw-settings-section is-wide">
          <DictationSettings />
        </section>
      )}
      <section className="rw-settings-section is-wide">
        <h3>{t('terminal.settings')}</h3>
        <TerminalSettingsForm />
      </section>
    </div>
  );
}

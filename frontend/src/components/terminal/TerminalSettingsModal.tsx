import React from 'react';
import { useT } from '../../shared/i18n';
import TerminalSettingsForm from './TerminalSettingsForm';
import DictationSettings from './dictation/DictationSettings';

/** The ⚙ dialog: terminal preferences, then dictation's. */
export default function TerminalSettingsModal({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  return (
    <div className="modal-overlay term-settings-overlay" onClick={onClose}>
      <div className="modal-content term-settings" onClick={(e) => e.stopPropagation()}>
        <h3>{t('terminal.settings')}</h3>
        <TerminalSettingsForm onDone={onClose} />
        <DictationSettings />
      </div>
    </div>
  );
}

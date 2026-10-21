import React from 'react';
import { useT } from '../../../shared/i18n';

/** Pauses the settings offer (ms); 0 is the older way: only "엔터" ends a sentence. */
export const PAUSES = [0, 800, 1000, 1500, 2000, 3000];

/** How long a pause ends what was said, for the speech service's end-of-speech detection. */
export default function PauseField({ id, value, onChange }: { id: string; value: number; onChange: (ms: number) => void }) {
  const { t } = useT();
  return (
    <div className="form-group">
      <label htmlFor={id}>{t('dictation.pause')}</label>
      <select id={id} className="select-input" value={PAUSES.includes(value) ? value : 1500} onChange={(e) => onChange(Number(e.target.value))}>
        {PAUSES.map((ms) => (
          <option key={ms} value={ms}>{ms === 0 ? t('dictation.pauseOff') : t('dictation.pauseSeconds', { seconds: (ms / 1000).toString() })}</option>
        ))}
      </select>
      <small className="text-secondary">{t('dictation.pauseHint')}</small>
    </div>
  );
}

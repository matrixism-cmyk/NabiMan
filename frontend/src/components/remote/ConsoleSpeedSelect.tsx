import React from 'react';
import { useT } from '../../shared/i18n';
import { ConsoleSpeed, SPEEDS } from './consoleSpeed';

/** The console bar's speed choice: 빠르게 / 균형 / 선명하게. */
export default function ConsoleSpeedSelect({ value, onChange }: { value: ConsoleSpeed; onChange: (next: ConsoleSpeed) => void }) {
  const { t } = useT();
  return (
    <select className="select-input rw-console-speed" value={value} title={t('remote.console.speed.hint')}
      aria-label={t('remote.console.speed.label')} onChange={(e) => onChange(e.target.value as ConsoleSpeed)}>
      {SPEEDS.map((speed) => <option key={speed} value={speed}>{t(`remote.console.speed.${speed}`)}</option>)}
    </select>
  );
}

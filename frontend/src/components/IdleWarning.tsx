import React from 'react';
import { useT } from '../i18n';

/** The minute before an idle sign-out: how long is left, and a way to stay. */
export default function IdleWarning({ seconds, onStay }: { seconds: number; onStay: () => void }) {
  const { t } = useT();
  return (
    <div className="idle-warning" role="alertdialog" aria-live="assertive" aria-label={t('idle.title')}>
      <span>{t('idle.warning', { seconds })}</span>
      <button type="button" className="btn btn-primary btn-sm" onClick={onStay}>{t('idle.stay')}</button>
    </div>
  );
}

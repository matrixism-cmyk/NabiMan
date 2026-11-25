import React, { useState, useSyncExternalStore } from 'react';
import { useT } from '../i18n';
import { currentNotices, dismissNotice, subscribeNotices } from './globalErrors';
import './error-boundary.css';

/** The corner where errors nothing else caught are told, without blocking. */
export default function GlobalErrorNotices() {
  const { t } = useT();
  const notices = useSyncExternalStore(subscribeNotices, currentNotices, currentNotices);
  const [copied, setCopied] = useState(0);
  const copy = (id: number, detail: string) => {
    // Refused by the browser: nothing to do here; the message itself is on screen.
    navigator.clipboard?.writeText(detail).then(() => setCopied(id)).catch(() => undefined);
  };
  if (!notices.length) return null;
  return (
    <div className="global-notices" role="status" aria-live="polite">
      {notices.map((notice) => (
        <div key={notice.id} className="global-notice">
          <span className="global-notice-text">
            {t('notice.unexpected', { message: notice.message })}
            {notice.count > 1 && ` ${t('notice.repeat', { count: notice.count })}`}
            <span className="global-notice-hint">{t('notice.unexpectedHint')}</span>
          </span>
          <button type="button" className="global-notice-copy" onClick={() => copy(notice.id, notice.detail)}>
            {copied === notice.id ? t('notice.copied') : t('crash.copy')}
          </button>
          <button type="button" className="global-notice-close" aria-label={t('common.close')}
            title={t('common.close')} onClick={() => dismissNotice(notice.id)}>×</button>
        </div>
      ))}
    </div>
  );
}

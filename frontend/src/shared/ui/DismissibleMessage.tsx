import React from 'react';
import { useT } from '../i18n';

/**
 * A panel's result line ("saved", "failed: …"). It is announced as a status,
 * a click on it puts it away as before, and its × does the same from the keyboard.
 */
export default function DismissibleMessage({ onDismiss, children }: { onDismiss: () => void; children: React.ReactNode }) {
  const { t } = useT();
  return (
    <div className="message message-dismissible" role="status" onClick={onDismiss}>
      <span>{children}</span>
      <button type="button" className="message-dismiss" aria-label={t('common.close')} title={t('common.close')}
        onClick={(e) => { e.stopPropagation(); onDismiss(); }}>×</button>
    </div>
  );
}

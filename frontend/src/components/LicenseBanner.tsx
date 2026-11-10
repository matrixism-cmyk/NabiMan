import React, { useState } from 'react';
import { useApi } from '../shared/api/useApi';
import { useT } from '../i18n';
import { LicenseStatus, noticeText } from './LicensePlan';

/**
 * A line under the header when a licence with a plan needs attention: limits
 * reached, expiry near, grace running or over. Nothing for any other licence.
 */
export default function LicenseBanner({ enabled, onOpen }: { enabled: boolean; onOpen: () => void }) {
  const { t } = useT();
  // Hourly: the state changes by day, and a reload shows changes at once.
  const { data } = useApi<LicenseStatus>(enabled ? '/api/license' : '', 3_600_000);
  const [hidden, setHidden] = useState(false);
  const notices = data?.notices ?? [];
  if (!enabled || hidden || !data || notices.length === 0) return null;
  const serious = notices.some(n => n === 'grace' || n === 'grace_ended' || n.endsWith('_over_limit'));
  return (
    <div className={`license-banner ${serious ? 'serious' : ''}`} role="status" aria-label={t('license.banner.label')}>
      <span>{notices.map(code => noticeText(t, code, data)).join(' ')}</span>
      <button type="button" className="btn btn-secondary btn-sm" onClick={onOpen}>{t('license.banner.open')}</button>
      <button type="button" className="btn btn-secondary btn-sm" onClick={() => setHidden(true)} aria-label={t('common.close')}>×</button>
    </div>
  );
}

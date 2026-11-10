import React from 'react';
import { useT } from '../i18n';

/** A plan's limits, as the signed licence carries them (`info.limits`). */
export interface PlanLimits {
  plan: string;
  deployment?: 'installed' | 'hosted';
  max_users?: number;
  max_servers?: number;
  features: string[];
  grace_days?: number;
}

/** What `/api/license` answers. The plan fields are null for licences without one. */
export interface LicenseStatus {
  active: boolean;
  tier: string;
  holder: string;
  expires_at: string | null;
  days_remaining: number | null;
  is_trial: boolean;
  trial_days_remaining: number | null;
  features: string[];
  max_servers: number | null;
  validation_state: string;
  limits?: PlanLimits | null;
  grace_ends_at?: string | null;
  grace_days_remaining?: number | null;
  usage?: { users: number; servers: number } | null;
  notices?: string[];
}

type TFn = (key: string) => string;

/** One notice code as a sentence, with the numbers it needs. */
export function noticeText(t: TFn, code: string, s: LicenseStatus): string {
  const limit = code.startsWith('users') ? s.limits?.max_users : s.limits?.max_servers;
  const count = code.startsWith('users') ? s.usage?.users : s.usage?.servers;
  return t(`license.notice.${code}`)
    .replace('{date}', (code === 'grace' ? s.grace_ends_at : s.expires_at) ?? '')
    .replace('{days}', String(s.days_remaining ?? ''))
    .replace('{max}', String(limit ?? ''))
    .replace('{count}', String(count ?? ''));
}

export function LicenseNotices({ status }: { status: LicenseStatus }) {
  const { t } = useT();
  const notices = status.notices ?? [];
  if (notices.length === 0) return null;
  return (
    <ul className="license-notices" role="alert">
      {notices.map(code => <li key={code}>{noticeText(t, code, status)}</li>)}
    </ul>
  );
}

/** The plan section of the licence screen: shown only for a licence that carries a plan. */
export function LicensePlanSection({ status }: { status: LicenseStatus }) {
  const { t } = useT();
  const plan = status.limits;
  if (!plan) return null;
  const used = (count: number | undefined, max: number | undefined) =>
    `${count ?? '-'} / ${max != null ? max : t('license.noLimit')}`;
  return (
    <>
      <h3>{t('license.plan')}</h3>
      <div className="info-grid">
        <div className="info-item">
          <span className="info-label">{t('license.plan')}</span>
          <span className="info-value">{plan.plan}</span>
        </div>
        {plan.deployment && (
          <div className="info-item">
            <span className="info-label">{t('license.deployment')}</span>
            <span className="info-value">{t(`license.deployment.${plan.deployment}`)}</span>
          </div>
        )}
        <div className="info-item">
          <span className="info-label">{t('license.maxUsers')}</span>
          <span className="info-value">{used(status.usage?.users, plan.max_users)}</span>
        </div>
        <div className="info-item">
          <span className="info-label">{t('license.planServers')}</span>
          <span className="info-value">{used(status.usage?.servers, plan.max_servers)}</span>
        </div>
        {status.grace_ends_at && (
          <div className="info-item">
            <span className="info-label">{t('license.graceEnds')}</span>
            <span className="info-value text-danger">{status.grace_ends_at}</span>
          </div>
        )}
      </div>
      <h3>{t('license.planFeatures')}</h3>
      <div className="info-grid">
        {['orchestration', 'graph_3d', 'recording', 'mec'].map(feature => {
          const on = plan.features.includes(feature);
          return (
            <div className="info-item" key={feature}>
              <span className="info-label">
                <span className={`status-badge ${on ? 'up' : 'down'}`}>{on ? t('license.enabled') : t('license.disabled')}</span>
              </span>
              <span className="info-value">{t(`license.feature.${feature}`)}</span>
            </div>
          );
        })}
      </div>
    </>
  );
}

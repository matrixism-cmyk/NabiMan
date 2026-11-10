import React, { useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { useApi } from '../../shared/api/useApi';
import { copyText } from '../../shared/ui/crashReport';
import { useTheme } from '../../shared/theme';
import type { Branding } from '../../shared/branding/branding';
import { useT } from '../../i18n';
import { AboutLicense, AboutRuntime, buildDiagnostics, contactHref } from './aboutInfo';
import './about.css';

interface Props {
  onClose: () => void;
  branding: Branding | null;
  /** Opens the licence screen; left out when this account cannot see it. */
  onOpenLicense?: () => void;
}

/** 정보: the product, its version and build, the licence, notices and who to ask. */
export default function AboutDialog({ onClose, branding, onOpenLicense }: Props) {
  const { t, lang } = useT();
  const { themeId } = useTheme();
  const { data: runtime, loading } = useApi<AboutRuntime>('/api/diagnostics/runtime');
  const { data: license } = useApi<AboutLicense>('/api/license');
  const [copied, setCopied] = useState<'' | 'ok' | 'fail'>('');
  const product = branding?.product_name || t('app.title');
  const unknown = loading ? t('common.loading') : t('about.unknown');
  const contact = branding?.noc_contact?.trim() || '';
  const href = contact ? contactHref(contact) : null;

  const licenseText = !license ? t('about.unknown')
    : license.is_trial && license.trial_days_remaining != null ? t('license.trialRemaining', { days: license.trial_days_remaining })
      : `${license.tier.toUpperCase()} · ${license.active ? t('license.active') : t('license.inactive')}`;

  const copy = async () => {
    const text = buildDiagnostics({ product, runtime, license, lang, theme: themeId });
    setCopied((await copyText(text)) ? 'ok' : 'fail');
  };

  return (
    <Modal onClose={onClose} title={t('about.title', { name: product })} className="about-dialog">
      <dl className="about-list">
        <dt>{t('about.version')}</dt><dd>{runtime?.package_version || unknown}</dd>
        <dt>{t('about.commit')}</dt><dd><code>{runtime?.git_commit || unknown}</code></dd>
        <dt>{t('tab.license')}</dt>
        <dd>
          {licenseText}
          {onOpenLicense && <> <button type="button" className="btn btn-secondary btn-sm"
            onClick={() => { onClose(); onOpenLicense(); }}>{t('about.openLicense')}</button></>}
        </dd>
        <dt>{t('about.notices')}</dt><dd>{t('about.noticesText')}</dd>
        <dt>{t('about.support')}</dt>
        <dd>{!contact ? <span className="text-secondary">{t('about.supportNone')}</span>
          : href ? <a href={href} target={href.startsWith('http') ? '_blank' : undefined} rel="noreferrer">{contact}</a>
            : contact}</dd>
      </dl>
      <p className="text-secondary about-hint">{t('about.copyHint')}</p>
      <div className="modal-actions">
        <button type="button" className="btn btn-primary" onClick={copy}>{t('about.copy')}</button>
        <button type="button" className="btn btn-secondary" onClick={onClose}>{t('common.close')}</button>
      </div>
      <p role="status" className="about-status">
        {copied === 'ok' ? t('about.copied') : copied === 'fail' ? t('about.copyFailed') : ''}
      </p>
    </Modal>
  );
}

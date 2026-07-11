import React from 'react';
import { useT } from '../../../i18n';
import { WizardState } from './WizardState';

interface Props {
  state: WizardState;
  setState: (s: WizardState) => void;
}

export default function Step1Basic({ state, setState }: Props) {
  const { t } = useT();
  return (
    <div>
      <h3>{t('mec.wizard.s1.title')}</h3>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        {t('mec.wizard.s1.displayName')}
        <input
          required
          value={state.displayName}
          onChange={(e) => setState({ ...state, displayName: e.target.value })}
          placeholder={t('mec.wizard.s1.displayNamePh')}
          style={{ width: '100%' }}
        />
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        {t('mec.wizard.s1.tenantId')}
        <input
          required
          value={state.tenantId}
          onChange={(e) =>
            setState({ ...state, tenantId: e.target.value.toLowerCase() })
          }
          placeholder="ygram-poc"
          pattern="[a-z][a-z0-9-]*[a-z0-9]"
          style={{ width: '100%' }}
        />
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        {t('mec.wizard.s1.taskName')}
        <input
          value={state.taskName}
          onChange={(e) => setState({ ...state, taskName: e.target.value })}
          placeholder={t('mec.wizard.s1.taskNamePh')}
          style={{ width: '100%' }}
        />
      </label>
      <label style={{ display: 'block', marginBottom: '8px' }}>
        {t('mec.wizard.s1.contactEmail')}
        <input
          type="email"
          value={state.contactEmail}
          onChange={(e) => setState({ ...state, contactEmail: e.target.value })}
          placeholder="contact@example.com"
          style={{ width: '100%' }}
        />
      </label>
    </div>
  );
}

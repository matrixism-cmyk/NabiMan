import React, { useState } from 'react';
import { useT } from '../../../i18n';
import { mecPost } from '../../../hooks/mec/useMecApi';
import { Tenant } from '../../../types/mec';
import Step1Basic from './Step1Basic';
import Step2Node from './Step2Node';
import Step3Quota from './Step3Quota';
import Step4Options from './Step4Options';
import {
  initialState,
  toRequestBody,
  validateStep1,
  validateStep2,
  validateStep3,
  WizardState,
} from './WizardState';

interface Props {
  onCreated: () => void | Promise<void>;
  onCancel: () => void;
}

export default function TenantWizard({ onCreated, onCancel }: Props) {
  const { t } = useT();
  const [step, setStep] = useState(1);
  const [state, setState] = useState<WizardState>(initialState());
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const next = () => {
    const validate = [validateStep1, validateStep2, validateStep3][step - 1];
    const msg = validate ? validate(state) : null;
    if (msg) {
      setErr(t(msg));
      return;
    }
    setErr(null);
    setStep(step + 1);
  };

  const back = () => {
    setErr(null);
    setStep(step - 1);
  };

  const submit = async () => {
    const msg =
      validateStep1(state) ||
      validateStep2(state) ||
      validateStep3(state);
    if (msg) {
      setErr(t(msg));
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const res = await mecPost<Tenant>(
        '/api/mec/v1/tenants',
        toRequestBody(state),
      );
      if (res.error) {
        setErr(res.error.message);
        return;
      }
      await onCreated();
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form
      className="panel-card"
      style={{ marginBottom: '16px' }}
      onSubmit={(e) => e.preventDefault()}
    >
      <div
        style={{
          display: 'flex',
          gap: '4px',
          marginBottom: '16px',
          fontSize: '12px',
          color: '#6b7280',
        }}
      >
        {[1, 2, 3, 4].map((s) => (
          <div
            key={s}
            style={{
              flex: 1,
              padding: '6px',
              textAlign: 'center',
              background: s === step ? '#3b82f6' : s < step ? '#d1fae5' : '#e5e7eb',
              color: s === step ? 'white' : '#374151',
              borderRadius: '3px',
              fontWeight: s === step ? 600 : 400,
            }}
          >
            {s}. {t(['mec.wizard.step.basic', 'mec.wizard.step.node', 'mec.wizard.step.quota', 'mec.wizard.step.options'][s - 1])}
          </div>
        ))}
      </div>

      {step === 1 && <Step1Basic state={state} setState={setState} />}
      {step === 2 && <Step2Node state={state} setState={setState} />}
      {step === 3 && <Step3Quota state={state} setState={setState} />}
      {step === 4 && <Step4Options state={state} setState={setState} />}

      {err && <div className="panel-error" style={{ marginTop: '12px' }}>{err}</div>}

      <div style={{ marginTop: '16px', display: 'flex', gap: '8px' }}>
        <button type="button" className="btn btn-secondary" onClick={onCancel}>
          {t('mec.wizard.cancel')}
        </button>
        <div style={{ flex: 1 }} />
        {step > 1 && (
          <button type="button" className="btn btn-secondary" onClick={back}>
            {t('mec.wizard.prev')}
          </button>
        )}
        {step < 4 && (
          <button type="button" className="btn btn-primary" onClick={next}>
            {t('mec.wizard.next')}
          </button>
        )}
        {step === 4 && (
          <button
            type="button"
            className="btn btn-primary"
            disabled={busy}
            onClick={submit}
          >
            {busy ? t('mec.wizard.creating') : t('mec.wizard.create')}
          </button>
        )}
      </div>
    </form>
  );
}

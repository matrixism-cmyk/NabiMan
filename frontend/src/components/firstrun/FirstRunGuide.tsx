import React, { useEffect, useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { apiRequest } from '../../shared/api/useApi';
import { useT } from '../../i18n';
import { STEPS, Step } from './firstRunState';
import './first-run.css';

interface Props {
  /** Steps already taken (from this browser's memory). */
  done: Step[];
  /** Steps this account cannot take here (their screen is hidden). */
  unavailable?: Step[];
  /** Takes one step: opens the screen it is about. */
  onStep: (step: Step) => void;
  onDismiss: () => void;
}

/**
 * A short checklist for a new install, floating over the corner of the page
 * so each step's screen can be used with it still in view. Closing it is
 * remembered for this account; the header menu brings it back.
 */
export default function FirstRunGuide({ done, unavailable = [], onStep, onDismiss }: Props) {
  const { t } = useT();
  const [guideOpen, setGuideOpen] = useState(false);
  const steps = STEPS.filter((s) => !unavailable.includes(s));
  const take = (step: Step) => {
    if (step === 'guide') setGuideOpen(true);
    onStep(step);
  };
  return (
    <aside className="first-run" aria-labelledby="first-run-title"
      onKeyDown={(e) => { if (e.key === 'Escape' && !guideOpen) { e.stopPropagation(); onDismiss(); } }}>
      <div className="first-run-head">
        <h2 id="first-run-title">{t('firstRun.title')}</h2>
        <button type="button" className="first-run-close" onClick={onDismiss}
          aria-label={t('firstRun.dismiss')} title={t('firstRun.dismiss')}>×</button>
      </div>
      <p className="first-run-intro">{t('firstRun.intro', { done: steps.filter((s) => done.includes(s)).length, total: steps.length })}</p>
      <ol className="first-run-steps">
        {steps.map((step) => {
          const isDone = done.includes(step);
          return <li key={step} className={isDone ? 'is-done' : ''}>
            <button type="button" className="first-run-step" onClick={() => take(step)}>
              <span className="first-run-mark" aria-hidden="true">{isDone ? '✓' : '○'}</span>
              <span>
                <strong>{t(`firstRun.${step}`)}</strong>
                <small>{t(`firstRun.${step}.hint`)}</small>
              </span>
              {isDone && <span className="sr-only">{t('firstRun.doneLabel')}</span>}
            </button>
          </li>;
        })}
      </ol>
      <div className="first-run-foot">
        <button type="button" className="btn btn-secondary btn-sm" onClick={onDismiss}>{t('firstRun.dismiss')}</button>
      </div>
      {guideOpen && <GuideDialog onClose={() => setGuideOpen(false)} />}
    </aside>
  );
}

/** The orchestration guide (GET /api/orchestration/guide), to read. */
function GuideDialog({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const [text, setText] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let live = true;
    apiRequest<{ text: string }>('/api/orchestration/guide')
      .then((res) => { if (!live) return; if (res.success && res.data) setText(res.data.text); else setFailed(true); })
      .catch(() => { if (live) setFailed(true); });
    return () => { live = false; };
  }, []);
  return (
    <Modal onClose={onClose} title={t('firstRun.guide')} className="first-run-guide">
      {failed ? <p role="alert">{t('firstRun.guideUnavailable')}</p>
        : text === null ? <p>{t('common.loading')}</p>
          : <pre className="first-run-guide-text">{text}</pre>}
      <div className="modal-actions">
        <button type="button" className="btn btn-secondary" onClick={onClose}>{t('common.close')}</button>
      </div>
    </Modal>
  );
}

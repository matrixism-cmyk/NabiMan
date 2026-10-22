import React, { useEffect, useState } from 'react';
import { apiRequest } from '../../../shared/api/useApi';
import { useT } from '../../../shared/i18n';

interface Voiceprint {
  enrolled: boolean;
  /** Enrolled under an older way of telling voices: to be enrolled again. */
  stale: boolean;
  /** The service's administrator asked for it to be enrolled again. */
  redo: boolean;
  last_used_at: string | null;
  use_count: number | null;
  /** The service's account page, where a voice is enrolled. */
  enrol_at: string;
}

/**
 * "내 목소리만": the service takes only the voice enrolled with its account —
 * the API key's owner, for every dictation here — and shows whether one is
 * enrolled. Enrolling (consent, reading sentences aloud) is done on the
 * service's own account page; nothing of the voice passes through NabiMan.
 */
export default function VoiceLockField({ value, onChange, keySet }: { value: boolean; onChange: (on: boolean) => void; keySet: boolean }) {
  const { t } = useT();
  const [print, setPrint] = useState<Voiceprint | null>(null);
  const [problem, setProblem] = useState('');

  useEffect(() => {
    if (!keySet) return;
    let live = true;
    apiRequest<Voiceprint>('/api/terminal/dictation/voiceprint').then((r) => {
      if (!live) return;
      if (r.success && r.data) setPrint(r.data);
      else setProblem(r.message || t('common.error'));
    }).catch(() => { if (live) setProblem(t('common.error')); });
    return () => { live = false; };
  }, [keySet, t]);

  const status = !keySet ? t('dictation.voiceNeedsKey')
    : problem ? problem
    : !print ? t('common.loading')
    : print.stale || print.redo ? t('dictation.voiceRedo')
    : print.enrolled ? t('dictation.voiceEnrolled', { count: print.use_count ?? 0 })
    : t('dictation.voiceNotEnrolled');
  const ready = Boolean(print?.enrolled && !print.stale && !print.redo);
  return (
    <div className="form-group">
      <label className="dictation-check">
        <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} />
        <span>{t('dictation.voiceLock')}</span>
      </label>
      <small className="text-secondary">{t('dictation.voiceLockSettingHint')}</small>
      <small className={`voice-status ${ready ? 'is-ready' : 'is-missing'}`} role="status">
        {status}
        {print && !ready && (
          <> · <a href={print.enrol_at} target="_blank" rel="noreferrer">{t('dictation.voiceEnrolLink')}</a></>
        )}
      </small>
      {value && print && !ready && <small className="text-warning">{t('dictation.voiceLockWithout')}</small>}
    </div>
  );
}

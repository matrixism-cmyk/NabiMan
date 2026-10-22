import React, { useEffect, useState } from 'react';
import { apiPost } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { useTerminalSettings } from './settings';
import { fontOf, TERMINAL_FONTS } from './terminalFonts';
import { PAUSES } from './dictation/PauseField';
import { DictationSetup, dictationSetup } from './dictation/sockets';
import { reloadSession, saveSessionOverrides, SessionOverrides, useSessionEntry } from './sessionSettings';

const TIMEOUTS = [0, 1800, 3600, 21600, 86400, 604800];

interface FieldProps {
  id: string;
  label: string;
  /** What the account's settings say, shown while this session follows them. */
  inherited: string;
  following: boolean;
  onFollow: (follow: boolean) => void;
  children: React.ReactNode;
}

/** One setting: follow the account's, or set it for this session alone. */
function Field({ id, label, inherited, following, onFollow, children }: FieldProps) {
  const { t } = useT();
  return (
    <div className="form-group session-field">
      <label htmlFor={id}>{label}</label>
      <label className="session-follow">
        <input type="checkbox" checked={following} onChange={(e) => onFollow(e.target.checked)} />
        <span>{t('terminal.follow', { value: inherited })}</span>
      </label>
      {!following && children}
    </div>
  );
}

/**
 * The ⚙ dialog's "this session" side: each setting follows the account's
 * until it is set here, and whether the session is kept alive (or when it
 * closes) is the session's own.
 */
export default function SessionSettingsForm({ sessionId, onDone }: { sessionId: string; onDone?: () => void }) {
  const { t } = useT();
  const { settings: base } = useTerminalSettings();
  const entry = useSessionEntry(sessionId);
  const [voice, setVoice] = useState<DictationSetup | null>(null);
  const [draft, setDraft] = useState<SessionOverrides>({});
  const [keep, setKeep] = useState<{ keepalive: boolean; timeout_secs: number } | null>(null);
  const [message, setMessage] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => { dictationSetup().then(setVoice); }, []);
  useEffect(() => {
    if (!entry) return;
    setDraft(entry.overrides);
    setKeep(entry.session);
  }, [entry]);

  const set = <K extends keyof SessionOverrides>(key: K, value: SessionOverrides[K]) => setDraft((d) => ({ ...d, [key]: value }));
  const follow = <K extends keyof SessionOverrides>(key: K, now: SessionOverrides[K]) => (on: boolean) => set(key, on ? null : now);
  const on = (v: boolean) => t(v ? 'terminal.on' : 'terminal.off');
  const seconds = (ms: number) => (ms === 0 ? t('dictation.pauseOff') : t('dictation.pauseSeconds', { seconds: String(ms / 1000) }));
  const lifetime = (secs: number) => (secs === 0 ? t('terminal.permanent') : secs >= 86400 ? t('terminal.days', { n: secs / 86400 }) : t('terminal.hours', { n: secs / 3600 }));

  const save = async (next: SessionOverrides) => {
    setSaving(true);
    let error = await saveSessionOverrides(sessionId, next);
    const was = entry?.session;
    if (!error && keep && was && keep.keepalive !== was.keepalive) {
      const r = await apiPost(`/api/terminal/sessions/${encodeURIComponent(sessionId)}/keepalive`, { keepalive: keep.keepalive });
      if (!r.success) error = r.message;
    }
    if (!error && keep && was && !keep.keepalive && keep.timeout_secs !== was.timeout_secs) {
      const r = await apiPost(`/api/terminal/sessions/${encodeURIComponent(sessionId)}/timeout`, { timeout_secs: keep.timeout_secs });
      if (!r.success) error = r.message;
    }
    await reloadSession(sessionId);
    setSaving(false);
    if (error) { setMessage(error); return; }
    setMessage(t('terminal.settingsSaved'));
    onDone?.();
  };

  if (!entry || !voice) return <p className="text-secondary">{t('common.loading')}</p>;
  const pause = draft.dictation_pause_ms ?? voice.endpoint_ms;
  return (
    <div className="term-settings-form session-settings-form">
      <p className="text-secondary session-settings-hint">{t('terminal.sessionHint')}</p>
      {message && <div role="alert" className="message" onClick={() => setMessage('')}>{message}</div>}

      <Field id="ss-font" label={t('terminal.font')} inherited={fontOf(base.font_family).label}
        following={draft.font_family == null} onFollow={follow('font_family', base.font_family)}>
        <select id="ss-font" className="select-input" value={fontOf(draft.font_family ?? base.font_family).id}
          onChange={(e) => set('font_family', e.target.value)}>
          {TERMINAL_FONTS.map((f) => <option key={f.id} value={f.id}>{f.id === 'system' ? t('terminal.fontSystem') : f.label}</option>)}
        </select>
      </Field>
      <Field id="ss-size" label={t('terminal.fontSize')} inherited={String(base.font_size)}
        following={draft.font_size == null} onFollow={follow('font_size', base.font_size)}>
        <input id="ss-size" type="number" min={8} max={28} className="filter-input" style={{ width: 120 }}
          value={draft.font_size ?? base.font_size}
          onChange={(e) => set('font_size', Math.min(28, Math.max(8, parseInt(e.target.value, 10) || base.font_size)))} />
      </Field>
      <Field id="ss-scroll" label={t('terminal.scrollbackLines')} inherited={base.scrollback_lines.toLocaleString()}
        following={draft.scrollback_lines == null} onFollow={follow('scrollback_lines', base.scrollback_lines)}>
        <input id="ss-scroll" type="number" min={100} max={200000} step={1000} className="filter-input" style={{ width: 140 }}
          value={draft.scrollback_lines ?? base.scrollback_lines}
          onChange={(e) => set('scrollback_lines', Math.min(200000, Math.max(100, parseInt(e.target.value, 10) || 100)))} />
      </Field>
      <Field id="ss-restore" label={t('terminal.restoreScrollback')} inherited={on(base.restore_scrollback)}
        following={draft.restore_scrollback == null} onFollow={follow('restore_scrollback', base.restore_scrollback)}>
        <label className="session-follow"><input id="ss-restore" type="checkbox" checked={draft.restore_scrollback ?? base.restore_scrollback}
          onChange={(e) => set('restore_scrollback', e.target.checked)} /><span>{t('terminal.restoreScrollback')}</span></label>
      </Field>
      {voice.available && (<>
        <Field id="ss-pause" label={t('dictation.pause')} inherited={seconds(voice.endpoint_ms)}
          following={draft.dictation_pause_ms == null} onFollow={follow('dictation_pause_ms', voice.endpoint_ms)}>
          <select id="ss-pause" className="select-input" value={PAUSES.includes(pause) ? pause : 1500}
            onChange={(e) => set('dictation_pause_ms', Number(e.target.value))}>
            {PAUSES.map((ms) => <option key={ms} value={ms}>{seconds(ms)}</option>)}
          </select>
        </Field>
        <Field id="ss-enter" label={t('dictation.autoEnter')} inherited={on(voice.auto_enter)}
          following={draft.dictation_auto_enter == null} onFollow={follow('dictation_auto_enter', voice.auto_enter)}>
          <label className="session-follow"><input id="ss-enter" type="checkbox" checked={draft.dictation_auto_enter ?? voice.auto_enter}
            onChange={(e) => set('dictation_auto_enter', e.target.checked)} /><span>{t('dictation.autoEnter')}</span></label>
        </Field>
      </>)}

      {keep && (
        <div className="form-group session-keep">
          <label className="session-follow">
            <input type="checkbox" checked={keep.keepalive} onChange={(e) => setKeep({ ...keep, keepalive: e.target.checked })} />
            <span>{t('terminal.keepalive')}</span>
          </label>
          {!keep.keepalive && (
            <select className="select-input" aria-label={t('terminal.idleTimeout')} value={keep.timeout_secs}
              onChange={(e) => setKeep({ ...keep, timeout_secs: Number(e.target.value) })}>
              {(TIMEOUTS.includes(keep.timeout_secs) ? TIMEOUTS : [...TIMEOUTS, keep.timeout_secs]).map((s) => (
                <option key={s} value={s}>{t('terminal.idleTimeout')}: {lifetime(s)}</option>
              ))}
            </select>
          )}
          <small className="text-secondary">{t('terminal.sessionKeepHint')}</small>
        </div>
      )}

      <div className="btn-group">
        <button type="button" className="btn btn-primary" disabled={saving} onClick={() => save(draft)}>{t('common.save')}</button>
        <button type="button" className="btn btn-secondary" disabled={saving} onClick={() => setDraft({})}>{t('terminal.sessionReset')}</button>
        {onDone && <button type="button" className="btn btn-secondary" onClick={onDone}>{t('common.cancel')}</button>}
      </div>
    </div>
  );
}

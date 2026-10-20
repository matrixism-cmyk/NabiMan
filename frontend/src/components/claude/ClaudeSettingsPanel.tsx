import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../../i18n';
import { getSettings, putSettings, speak, testClaude, VoiceSettings, VoiceStatus } from './claudeApi';
import { Speaker } from './speaker';
import './claude.css';

const SAMPLE = '안녕하세요. 이 목소리와 빠르기로 답변을 읽어 드릴게요.';

/**
 * How Claude talks: the voice, its manner and speed (tried out before
 * saving), whether listening goes on after an answer, and how Claude runs
 * here — with no tools, or with tools that only read.
 */
export default function ClaudeSettingsPanel() {
  const { t } = useT();
  const [s, setS] = useState<VoiceSettings | null>(null);
  const [status, setStatus] = useState<VoiceStatus | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState('');
  const [trying, setTrying] = useState(false);
  const speaker = useRef<Speaker | null>(null);

  useEffect(() => {
    getSettings().then((res) => {
      if (res.success && res.data) { setS(res.data.settings); setStatus(res.data.status); }
      else setMessage(res.message || t('common.error'));
    });
    return () => speaker.current?.stop();
  }, [t]);

  if (!s || !status) return <div className="panel">{message || t('common.loading')}</div>;
  const set = <K extends keyof VoiceSettings>(key: K, value: VoiceSettings[K]) => setS({ ...s, [key]: value });

  const save = async () => {
    setBusy('save');
    const res = await putSettings(s);
    setBusy('');
    if (res.success && res.data) { setS(res.data.settings); setStatus(res.data.status); setMessage(t('dictation.saved')); }
    else setMessage(res.message || t('common.error'));
  };

  const tryVoice = () => {
    speaker.current?.stop();
    speaker.current = new Speaker((text, signal) => speak(text, signal, { speaker: s.speaker, instruct: s.instruct }),
      setTrying, (m) => setMessage(m));
    speaker.current.rate = s.speed;
    speaker.current.unlock();
    speaker.current.say(SAMPLE);
  };

  const test = async () => {
    setBusy('test');
    const res = await testClaude();
    setBusy('');
    setMessage(res.success && res.data ? t('claude.testOk', { version: res.data.version, user: res.data.run_as }) : res.message || t('common.error'));
  };

  return (
    <div className="panel cv-settings">
      <h2>{t('claude.settings')}</h2>
      {message && <div className="message" role="status">{message}</div>}

      <section>
        <h3>{t('claude.voiceSection')}</h3>
        {!status.voice_ready && <p className="message" role="alert">{t('claude.noVoice')}</p>}
        <label className="cv-field">
          <span>{t('claude.voice')}</span>
          <select className="select-input" value={s.speaker} onChange={(e) => set('speaker', e.target.value)}>
            {status.speakers.map((v) => <option key={v.id} value={v.id}>{v.name} — {v.desc}</option>)}
          </select>
        </label>
        <label className="cv-field">
          <span>{t('claude.speed')} <strong>{s.speed.toFixed(1)}×</strong></span>
          <input type="range" min={0.5} max={2} step={0.1} value={s.speed} onChange={(e) => set('speed', Number(e.target.value))} />
        </label>
        <label className="cv-field">
          <span>{t('claude.manner')}</span>
          <input className="filter-input" maxLength={100} value={s.instruct} placeholder={t('claude.mannerHint')}
            onChange={(e) => set('instruct', e.target.value)} />
        </label>
        <label className="cv-field">
          <span>{t('claude.language')}</span>
          <select className="select-input" value={s.language} onChange={(e) => set('language', e.target.value)}>
            {['korean', 'english', 'japanese', 'chinese', 'auto'].map((l) => <option key={l} value={l}>{t(`claude.lang.${l}`)}</option>)}
          </select>
        </label>
        <button type="button" className="btn btn-secondary" onClick={trying ? () => speaker.current?.stop() : tryVoice} disabled={!status.voice_ready}>
          {trying ? `■ ${t('claude.stop')}` : `🔊 ${t('claude.tryVoice')}`}
        </button>
      </section>

      <section>
        <h3>{t('claude.talkSection')}</h3>
        <label className="cv-check">
          <input type="checkbox" checked={s.continuous} onChange={(e) => set('continuous', e.target.checked)} />
          <span>{t('claude.continuous')}</span>
        </label>
        <p className="text-secondary">{t('claude.enterHint')}</p>
      </section>

      <section>
        <h3>{t('claude.claudeSection')}</h3>
        <label className="cv-field">
          <span>{t('claude.tools')}</span>
          <select className="select-input" value={s.tools} onChange={(e) => set('tools', e.target.value as VoiceSettings['tools'])}>
            <option value="none">{t('claude.tools.none')}</option>
            <option value="read">{t('claude.tools.read')}</option>
          </select>
        </label>
        <label className="cv-field">
          <span>{t('claude.model')}</span>
          <input className="filter-input" value={s.model} placeholder={t('claude.modelHint')} onChange={(e) => set('model', e.target.value)} />
        </label>
        <label className="cv-field">
          <span>{t('claude.extraPrompt')}</span>
          <textarea className="filter-input" rows={3} maxLength={2000} value={s.extra_prompt} placeholder={t('claude.extraPromptHint')}
            onChange={(e) => set('extra_prompt', e.target.value)} />
        </label>
        <details>
          <summary>{t('claude.advanced')}</summary>
          <label className="cv-field">
            <span>{t('claude.runAs')}</span>
            <input className="filter-input" value={s.run_as} placeholder={status.run_as || ''} onChange={(e) => set('run_as', e.target.value)} />
          </label>
          <label className="cv-field">
            <span>{t('claude.path')}</span>
            <input className="filter-input" value={s.claude_path} placeholder={status.claude || ''} onChange={(e) => set('claude_path', e.target.value)} />
          </label>
        </details>
        <p className="text-secondary">
          {status.problem || t('claude.found', { path: status.claude || '', user: status.run_as || '' })}
        </p>
        <button type="button" className="btn btn-secondary" onClick={test} disabled={busy !== ''}>{t('claude.test')}</button>
      </section>

      <div className="btn-group">
        <button type="button" className="btn btn-primary" onClick={save} disabled={busy !== ''}>{t('common.save')}</button>
      </div>
    </div>
  );
}

import React, { useEffect, useState } from 'react';
import './dictation.css';
import { apiPost, apiRequest } from '../../../hooks/useApi';
import { useT } from '../../../i18n';
import { forgetDictationAvailability } from './sockets';
import PauseField from './PauseField';

interface Setup {
  available: boolean;
  language: string;
  url?: string;
  address?: string;
  key_set?: boolean;
  key_hint?: string | null;
  endpoint_ms?: number;
  auto_enter?: boolean;
}

const LANGUAGES = ['korean', 'english', 'japanese', 'chinese', 'auto'];

/**
 * Where terminal dictation is transcribed, for a super administrator: the
 * speech service's address, an address to reach it by from this server,
 * the API key (kept encrypted here; never shown again), and the language.
 * Others never see this section.
 */
export default function DictationSettings() {
  const { t } = useT();
  const [setup, setSetup] = useState<Setup | null>(null);
  const [url, setUrl] = useState('');
  const [address, setAddress] = useState('');
  const [language, setLanguage] = useState('korean');
  const [pause, setPause] = useState(1500);
  const [autoEnter, setAutoEnter] = useState(false);
  const [key, setKey] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);

  const show = (s: Setup) => {
    setSetup(s);
    setUrl(s.url || '');
    setAddress(s.address || '');
    setLanguage(s.language || 'korean');
    setPause(s.endpoint_ms ?? 1500);
    setAutoEnter(Boolean(s.auto_enter));
  };

  useEffect(() => {
    apiRequest<Setup>('/api/terminal/dictation/settings').then((r) => { if (r.success && r.data) show(r.data); });
  }, []);

  if (!setup || setup.url === undefined) return null;

  const save = async (clearKey = false) => {
    setBusy(true);
    setMessage('');
    const body: Record<string, string | number | boolean> = { url, address, language, endpoint_ms: pause, auto_enter: autoEnter };
    if (key.trim() || clearKey) body.key = clearKey ? '' : key.trim();
    const r = await apiRequest<Setup>('/api/terminal/dictation/settings', { method: 'PUT', body: JSON.stringify(body) });
    setBusy(false);
    if (!r.success || !r.data) { setMessage(r.message || t('common.error')); return; }
    show(r.data);
    setKey('');
    forgetDictationAvailability();
    setMessage(t('dictation.saved'));
  };

  const test = async () => {
    setBusy(true);
    setMessage(t('dictation.testing'));
    const r = await apiPost<{ ready?: boolean }>('/api/terminal/dictation/test', {});
    setBusy(false);
    setMessage(r.success ? t(r.data?.ready === false ? 'dictation.testNotReady' : 'dictation.testOk') : r.message || t('common.error'));
  };

  return (
    <section className="dictation-settings" aria-labelledby="dictation-settings-title">
      <h4 id="dictation-settings-title">{t('dictation.settingsTitle')}</h4>
      <p className="text-secondary">{t('dictation.settingsHint')}</p>
      {message && <p role="status" className="message">{message}</p>}
      <div className="form-group">
        <label htmlFor="dictation-url">{t('dictation.url')}</label>
        <input id="dictation-url" className="filter-input" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://v.bebo.kr" />
      </div>
      <div className="form-group">
        <label htmlFor="dictation-address">{t('dictation.address')}</label>
        <input id="dictation-address" className="filter-input" value={address} onChange={(e) => setAddress(e.target.value)} placeholder="127.0.0.1:443" />
        <small className="text-secondary">{t('dictation.addressHint')}</small>
      </div>
      <div className="form-group">
        <label htmlFor="dictation-key">{t('dictation.key')}</label>
        <input id="dictation-key" className="filter-input" type="password" autoComplete="off" value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder={setup.key_set ? `${t('dictation.keyKeep')} (${setup.key_hint})` : 'vk_…'} />
        <small className="text-secondary">{t('dictation.keyHint')}</small>
      </div>
      <div className="form-group">
        <label htmlFor="dictation-language">{t('dictation.language')}</label>
        <select id="dictation-language" className="select-input" value={language} onChange={(e) => setLanguage(e.target.value)}>
          {LANGUAGES.map((l) => <option key={l} value={l}>{t(`dictation.lang.${l}`)}</option>)}
        </select>
      </div>
      <PauseField id="dictation-pause" value={pause} onChange={setPause} />
      {pause > 0 && (
        <div className="form-group">
          <label><input type="checkbox" checked={autoEnter} onChange={(e) => setAutoEnter(e.target.checked)} /> {t('dictation.autoEnter')}</label>
          <small className="text-secondary">{t('dictation.autoEnterHint')}</small>
        </div>
      )}
      <div className="btn-group">
        <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => save()}>{t('common.save')}</button>
        <button type="button" className="btn btn-secondary btn-sm" disabled={busy || !setup.key_set} onClick={test}>{t('dictation.test')}</button>
        {setup.key_set && (
          <button type="button" className="btn btn-secondary btn-sm" disabled={busy} onClick={() => save(true)}>{t('dictation.clearKey')}</button>
        )}
      </div>
    </section>
  );
}

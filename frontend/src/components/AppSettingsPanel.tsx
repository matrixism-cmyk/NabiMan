import React, { useEffect, useState } from 'react';
import { useT } from '../i18n';
import { apiRequest } from '../shared/api/useApi';

const CHOICES = [0, 5, 10, 15, 30, 60, 120, 240, 480, 1440];

/** NabiMan's own settings (환경설정): for now, signing out after a time without use. */
export default function AppSettingsPanel() {
  const { t } = useT();
  const [minutes, setMinutes] = useState<number | null>(null);
  const [saved, setSaved] = useState<number | null>(null);
  const [message, setMessage] = useState('');

  useEffect(() => {
    apiRequest<{ idle_logout_minutes: number }>('/api/app-settings').then((r) => {
      if (r.success && r.data) { setMinutes(r.data.idle_logout_minutes); setSaved(r.data.idle_logout_minutes); }
      else setMessage(r.message || t('common.error'));
    });
  }, [t]);

  const label = (m: number) => (m === 0 ? t('idle.never')
    : m % 60 === 0 ? t('idle.hours', { count: m / 60 }) : t('idle.minutes', { count: m }));

  const save = async () => {
    if (minutes === null) return;
    const r = await apiRequest<{ idle_logout_minutes: number }>('/api/app-settings', {
      method: 'PUT', body: JSON.stringify({ idle_logout_minutes: minutes }),
    });
    if (r.success && r.data) {
      setSaved(r.data.idle_logout_minutes);
      setMessage(t('idle.saved'));
      window.dispatchEvent(new Event('nabiman_app_settings_changed'));
    } else setMessage(r.message || t('common.error'));
  };

  if (minutes === null) return <div className="panel">{message || t('common.loading')}</div>;
  return (
    <div className="panel app-settings">
      <h2>{t('tab.appSettings')}</h2>
      {message && <div className="message" role="status">{message}</div>}
      <section className="app-settings-section">
        <h3>{t('idle.title')}</h3>
        <p className="text-secondary">{t('idle.explain')}</p>
        <label className="app-settings-field">
          <span>{t('idle.after')}</span>
          <select className="select-input" value={minutes} onChange={(e) => setMinutes(Number(e.target.value))}>
            {CHOICES.map((m) => <option key={m} value={m}>{label(m)}</option>)}
          </select>
        </label>
        <p className="text-secondary">{t('idle.restart')}</p>
        <div className="btn-group">
          <button type="button" className="btn btn-primary" onClick={save} disabled={minutes === saved}>{t('common.save')}</button>
        </div>
      </section>
    </div>
  );
}

import React, { useState } from 'react';
import './registration-form.css';
import { useAuthCooldown } from '../hooks/useAuthCooldown';
import { useT } from '../i18n';
import { PASSWORD_MIN } from '../shared/passwordRule';

export default function RegistrationForm({ onBack }: { onBack: () => void }) {
  const cooldown = useAuthCooldown();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [message, setMessage] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const { t } = useT();
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (cooldown.seconds || busy) return;
    if (password !== confirm) { setMessage(t('signup.mismatch')); return; }
    setBusy(true); setMessage('');
    try {
      const res = await fetch('/api/auth/register', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      });
      if (cooldown.accept(res)) return;
      const json = await res.json();
      setMessage(json.message || json.data || t('signup.failed'));
      if (json.success) { setSubmitted(true); setPassword(''); setConfirm(''); setMessage(json.data); }
    } catch { setMessage(t('signup.offline')); }
    finally { setBusy(false); }
  };
  return <div className="login-screen"><form className="login-form registration-form" onSubmit={submit}>
    <h1>{t('signup.title')}</h1>
    <p>{t('signup.lead')}</p>
    {message && <div role="status" className="message">{message}</div>}
    {cooldown.seconds > 0 && <p role="status">{t('signup.cooldown', { seconds: cooldown.seconds })}</p>}
    {!submitted && <>
      <label>{t('signup.id')}<input aria-label={t('signup.idLabel')} autoComplete="username" required minLength={2} maxLength={32}
        value={username} onChange={e => setUsername(e.target.value)} pattern="[A-Za-z0-9_-]+" /></label>
      <label>{t('signup.password')}<input aria-label={t('signup.passwordLabel')} type="password" autoComplete="new-password" required minLength={PASSWORD_MIN}
        value={password} onChange={e => setPassword(e.target.value)} /></label>
      <label>{t('signup.confirm')}<input aria-label={t('signup.confirm')} type="password" autoComplete="new-password" required
        value={confirm} onChange={e => setConfirm(e.target.value)} /></label>
      <button className="btn btn-primary" disabled={busy || cooldown.seconds > 0}>{busy ? t('signup.submitting') : t('signup.submit')}</button>
    </>}
    <button type="button" className="btn btn-secondary" onClick={onBack}>{t('signup.back')}</button>
  </form></div>;
}

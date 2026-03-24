import React, { useState } from 'react';
import { setToken, setRefreshToken } from '../hooks/useApi';
import { useT, LANG_LABELS, Lang } from '../i18n';

interface Props {
  onLogin: () => void;
}

export default function LoginScreen({ onLogin }: Props) {
  const { t, lang, setLang } = useT();
  const [username, setUsername] = useState('admin');
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [needs2fa, setNeeds2fa] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      if (needs2fa) {
        const res = await fetch('/api/auth/2fa/verify', {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, code: totpCode }),
        });
        const json = await res.json();
        if (json.success && json.data?.token) {
          setToken(json.data.token);
          if (json.data.refresh_token) setRefreshToken(json.data.refresh_token);
          onLogin();
        } else { setError(json.message || t('login.failed')); }
      } else {
        const res = await fetch('/api/auth/login', {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, password }),
        });
        const json = await res.json();
        if (json.success && json.data?.requires_2fa) {
          setNeeds2fa(true); setTotpCode('');
        } else if (json.success && json.data?.token) {
          setToken(json.data.token);
          if (json.data.refresh_token) setRefreshToken(json.data.refresh_token);
          onLogin();
        } else { setError(json.message || t('login.failed')); }
      }
    } catch { setError(t('login.connectionFailed')); }
    finally { setLoading(false); }
  };

  return (
    <div className="login-screen">
      <form className="login-form" onSubmit={handleSubmit}>
        <h1>{t('app.title')}</h1>
        <p className="login-subtitle">{t('app.subtitle')}</p>
        {error && <div className="login-error">{error}</div>}
        {!needs2fa ? (<>
          <input type="text" placeholder={t('login.username')} value={username}
            onChange={e => setUsername(e.target.value)} autoFocus required />
          <input type="password" placeholder={t('login.password')} value={password}
            onChange={e => setPassword(e.target.value)} required />
        </>) : (
          <input type="text" placeholder={t('login.totpCode')} value={totpCode}
            onChange={e => setTotpCode(e.target.value)} autoFocus required
            maxLength={6} style={{ textAlign: 'center', fontSize: '24px', letterSpacing: '8px' }} />
        )}
        <button type="submit" className="btn btn-primary" disabled={loading}>
          {loading ? t('login.loggingIn') : t('login.button')}
        </button>
        <select
          value={lang}
          onChange={e => setLang(e.target.value as Lang)}
          className="lang-select"
          style={{ marginTop: '12px', alignSelf: 'center' }}
        >
          {(Object.entries(LANG_LABELS) as [Lang, string][]).map(([code, label]) => (
            <option key={code} value={code}>{label}</option>
          ))}
        </select>
      </form>
    </div>
  );
}

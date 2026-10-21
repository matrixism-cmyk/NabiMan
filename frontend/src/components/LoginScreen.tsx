import RegistrationForm from './RegistrationForm';
import { useAuthCooldown } from '../hooks/useAuthCooldown';
import React, { useState, useEffect } from 'react';
import { setToken, setRefreshToken } from '../hooks/useApi';
import { useT, LANG_LABELS, Lang } from '../i18n';

interface Props {
  onLogin: () => void;
}

type LoginMode = 'local' | 'ldap';

export default function LoginScreen({ onLogin }: Props) {
  const { t, lang, setLang } = useT();
  const cooldown = useAuthCooldown();
  const [registering, setRegistering] = useState(window.location.pathname === '/signup');
  const [username, setUsername] = useState('admin');
  const [password, setPassword] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [needs2fa, setNeeds2fa] = useState(false);
  const [error, setError] = useState('');
  const [securityChanged] = useState(() => sessionStorage.getItem('nabiman_security_changed') === '1');
  const [reauthRequired] = useState(() => sessionStorage.getItem('nabiman_reauth_required') === '1');
  const [idleSignedOut] = useState(() => sessionStorage.getItem('nabiman_idle_logout') === '1');
  const [loading, setLoading] = useState(false);
  const [loginMode, setLoginMode] = useState<LoginMode>('local');
  const [ldapEnabled, setLdapEnabled] = useState(false);
  const [oauthEnabled, setOauthEnabled] = useState(false);
  const [oauthProvider, setOauthProvider] = useState('');

  useEffect(() => {
    const syncPage = () => setRegistering(window.location.pathname === '/signup');
    window.addEventListener('popstate', syncPage);
    return () => window.removeEventListener('popstate', syncPage);
  }, []);

  useEffect(() => {
    fetch('/api/auth/ldap/provider').then(r => r.json()).then(j => {
      if (j.success && j.data?.enabled) setLdapEnabled(true);
    }).catch(() => {});
    fetch('/api/auth/oauth/provider').then(r => r.json()).then(j => {
      if (j.success && j.data?.enabled) {
        setOauthEnabled(true);
        setOauthProvider(j.data.provider_name || 'SSO');
      }
    }).catch(() => {});
  }, []);

  const handleOAuthLogin = async () => {
    setLoading(true);
    try {
      const res = await fetch('/api/auth/oauth/authorize');
      const json = await res.json();
      if (json.success && json.data?.url) {
        window.location.href = json.data.url;
      } else { setError(json.message || 'OAuth error'); }
    } catch { setError(t('login.connectionFailed')); }
    finally { setLoading(false); }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (cooldown.seconds || loading) return;
    setLoading(true);
    setError('');
    try {
      if (needs2fa) {
        const res = await fetch('/api/auth/2fa/verify', {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, password, code: totpCode }),
        });
        if (cooldown.accept(res)) return;
        const json = await res.json();
        if (json.success && json.data?.token) {
          setToken(json.data.token);
          if (json.data.refresh_token) setRefreshToken(json.data.refresh_token);
          onLogin();
        } else { setError(json.message || t('login.failed')); }
      } else {
        const endpoint = loginMode === 'ldap' ? '/api/auth/ldap/login' : '/api/auth/login';
        const res = await fetch(endpoint, {
          method: 'POST', headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username, password }),
        });
        if (cooldown.accept(res)) return;
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

  // Handle OAuth callback
  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const code = params.get('code');
    if (code) {
      window.history.replaceState({}, '', window.location.pathname);
      fetch(`/api/auth/oauth/callback?code=${encodeURIComponent(code)}&state=${encodeURIComponent(params.get('state') || '')}`)
        .then(r => r.json()).then(json => {
          if (json.success && json.data?.token) {
            setToken(json.data.token);
            if (json.data.refresh_token) setRefreshToken(json.data.refresh_token);
            onLogin();
          } else { setError(json.message || 'OAuth login failed'); }
        }).catch(() => setError('OAuth callback failed'));
    }
  }, [onLogin]);

  if (registering) return <RegistrationForm onBack={() => { window.history.pushState(null, '', '/'); setRegistering(false); }} />;

  return (
    <div className="login-screen">
      <form className="login-form" onSubmit={handleSubmit}>
        <img className="login-mark" src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="64" height="64" />
        <h1>{t('app.title')}</h1>
        <p className="login-subtitle">{t('app.subtitle')}</p>
        {securityChanged && <p role="status">{t('mfa.changed')}</p>}
        {reauthRequired && <p role="alert">{t('login.sessionEnded')}</p>}
        {idleSignedOut && <p role="status">{t('idle.signedOut')}</p>}
        {error && <div className="login-error">{error}</div>}
        {cooldown.seconds > 0 && <p role="status">{t('login.retryAfter').replace('{seconds}', String(cooldown.seconds))}</p>}

        {ldapEnabled && !needs2fa && (
          <div className="btn-group" style={{ marginBottom: '12px', justifyContent: 'center' }}>
            <button type="button" className={`btn btn-sm ${loginMode === 'local' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => setLoginMode('local')}>{t('login.local')}</button>
            <button type="button" className={`btn btn-sm ${loginMode === 'ldap' ? 'btn-primary' : 'btn-secondary'}`}
              onClick={() => { setLoginMode('ldap'); setUsername(''); }}>{t('login.ldap')}</button>
          </div>
        )}

        {!needs2fa ? (<>
          <input type="text" aria-label={t('login.username')} autoComplete="username" placeholder={loginMode === 'ldap' ? t('login.ldapUsername') : t('login.username')}
            value={username} onChange={e => setUsername(e.target.value)} autoFocus required />
          <input type="password" aria-label={t('login.password')} autoComplete="current-password" placeholder={t('login.password')} value={password}
            onChange={e => setPassword(e.target.value)} required />
        </>) : (
          <input type="text" aria-label={t('login.totpCode')} autoComplete="one-time-code" placeholder={t('login.totpCode')} value={totpCode}
            onChange={e => setTotpCode(e.target.value)} autoFocus required
            maxLength={64} style={{ textAlign: 'left', fontSize: '16px' }} />
        )}
        <button type="submit" className="btn btn-primary" disabled={loading || cooldown.seconds > 0}>
          {loading ? t('login.loggingIn') : t('login.button')}
        </button>

        {oauthEnabled && !needs2fa && (
          <button type="button" className="btn btn-secondary" onClick={handleOAuthLogin}
            disabled={loading} style={{ marginTop: '8px' }}>
            {oauthProvider} {t('login.oauthLogin')}
          </button>
        )}

        {!needs2fa && <button type="button" className="btn btn-secondary" onClick={() => { window.history.pushState(null, '', '/signup'); setRegistering(true); }}>회원가입 신청</button>}

        <select value={lang} onChange={e => setLang(e.target.value as Lang)}
          className="lang-select" style={{ marginTop: '12px', alignSelf: 'center' }}>
          {(Object.entries(LANG_LABELS) as [Lang, string][]).map(([code, label]) => (
            <option key={code} value={code}>{label}</option>
          ))}
        </select>
      </form>
    </div>
  );
}

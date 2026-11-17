import React, { useState } from 'react';
import { setRefreshToken, setToken } from '../shared/api/useApi';
import { RequestOptionsJSON, getPasskey, passkeyCancelled, passkeysSupported } from '../shared/auth/passkey';
import { useT } from '../i18n';

interface Props {
  onLogin: () => void;
  onError: (message: string) => void;
  /** Takes a 429 (the sign-in rate limit) and starts the wait; true when it did. */
  limited: (response: Response) => boolean;
  disabled?: boolean;
}

const post = (path: string, body: object) => fetch(path, {
  method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body),
});

/** "패스키로 로그인": shown only where the browser has passkeys. No username is asked for. */
export default function PasskeyLoginButton({ onLogin, onError, limited, disabled }: Props) {
  const { t } = useT();
  const [busy, setBusy] = useState(false);
  if (!passkeysSupported()) return null;

  const signIn = async () => {
    setBusy(true); onError('');
    try {
      const begun = await post('/api/auth/passkeys/login/begin', {});
      if (limited(begun)) return;
      if (begun.status === 503) { onError(t('passkey.notConfigured')); return; }
      const challenge = await begun.json();
      if (!challenge.success || !challenge.data) { onError(challenge.message || t('login.failed')); return; }
      const credential = await getPasskey(challenge.data.publicKey as RequestOptionsJSON);
      const res = await post('/api/auth/passkeys/login/finish', { challenge_id: challenge.data.challenge_id, credential });
      if (limited(res)) return;
      const json = await res.json();
      if (json.success && json.data?.token) {
        setToken(json.data.token);
        if (json.data.refresh_token) setRefreshToken(json.data.refresh_token);
        onLogin();
      } else {
        // 401 is one answer for every refusal; an account that is off (403) says why itself.
        onError(res.status === 401 ? t('passkey.loginFailed') : json.message || t('login.failed'));
      }
    } catch (cause) {
      onError(passkeyCancelled(cause) ? t('passkey.cancelled') : t('login.connectionFailed'));
    } finally { setBusy(false); }
  };

  return <button type="button" className="btn btn-secondary" onClick={signIn} disabled={busy || disabled} style={{ marginTop: '8px' }}>
    {busy ? t('passkey.waiting') : t('passkey.login')}
  </button>;
}

import React, { useCallback, useEffect, useRef, useState } from 'react';
import TerminalView, { TerminalViewHandle } from './TerminalView';
import { DEFAULT_TERMINAL_SETTINGS } from './settings';
import { apiPost, apiRequest } from '../../hooks/useApi';
import { useT } from '../../i18n';
import './terminal-windows.css';

interface ShareInfo {
  label: string;
  needs_password: boolean;
  read_only: boolean;
  expires_unix: number;
  alive: boolean;
}

/**
 * What someone sees when they open a share link: no NabiMan account, no
 * navigation — the shared terminal and nothing else.
 */
export default function SharePage({ token }: { token: string }) {
  const { t } = useT();
  const viewRef = useRef<TerminalViewHandle>(null);
  const [info, setInfo] = useState<ShareInfo | null>(null);
  const [error, setError] = useState('');
  const [password, setPassword] = useState('');
  const [ticket, setTicket] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  const load = useCallback(async () => {
    const res = await apiRequest<ShareInfo>(`/api/share/${encodeURIComponent(token)}`);
    if (res.success && res.data) {
      setInfo(res.data);
      if (!res.data.needs_password) setTicket('');
    } else {
      setError(res.message || t('share.invalid'));
    }
  }, [token, t]);

  useEffect(() => { load(); }, [load]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setChecking(true);
    const res = await apiPost<{ ticket: string }>(`/api/share/${encodeURIComponent(token)}/auth`, { password });
    setChecking(false);
    if (res.success && res.data) { setTicket(res.data.ticket); setError(''); }
    else setError(res.message || t('share.wrongPassword'));
  };

  if (error && !info) {
    return (
      <div className="share-page share-page-center">
        <div className="share-gate">
          <h1>{t('share.invalidTitle')}</h1>
          <p className="text-secondary">{error}</p>
        </div>
      </div>
    );
  }

  if (!info) {
    return <div className="share-page share-page-center"><p className="text-secondary">{t('common.loading')}</p></div>;
  }

  if (ticket === null) {
    return (
      <div className="share-page share-page-center">
        <form className="share-gate" onSubmit={submit}>
          <h1>{t('share.gateTitle')}</h1>
          <p className="text-secondary">{info.label}</p>
          {error && <div className="login-error">{error}</div>}
          <input
            type="password" value={password} autoFocus autoComplete="current-password"
            placeholder={t('share.passwordPrompt')}
            onChange={(e) => setPassword(e.target.value)}
          />
          <button className="btn btn-primary" type="submit" disabled={checking}>
            {checking ? t('common.working') : t('share.enter')}
          </button>
        </form>
      </div>
    );
  }

  return (
    <div className="share-page">
      <header className="share-head">
        <strong>{t('share.headTitle')}</strong>
        <code>{info.label}</code>
        {info.read_only && <span className="status-badge">{t('share.readOnly')}</span>}
        {!info.alive && <span className="status-badge down">{t('share.ended')}</span>}
        <span className="share-head-spacer" />
        <button className="btn btn-secondary btn-sm" onClick={() => viewRef.current?.reconnect()}>
          {t('terminal.reconnect')}
        </button>
      </header>
      <div className="share-body">
        <TerminalView
          ref={viewRef}
          target={{ kind: 'local' }}
          storageKey={`share_${token}`}
          settings={DEFAULT_TERMINAL_SETTINGS}
          share={{ token, ticket }}
          active
        />
      </div>
    </div>
  );
}

import React, { useEffect } from 'react';
import { useT } from '../../i18n';
import { ThemeSelector } from '../../theme';
import LanguageSelector from '../LanguageSelector';
import RemoteWorkspace from './RemoteWorkspace';
import { useChromeHidden } from '../../hooks/useChromeHidden';
import './remote-shell.css';

/** Whether this page was opened as the remote workspace on its own (/remote). */
export const isStandaloneRemote = () => /^\/remote\/?$/.test(window.location.pathname);

interface Props {
  username: string;
  /** Administrators only, as in NabiMan's own menu. */
  allowed: boolean;
  onLogout: () => void;
}

/**
 * The remote workspace on its own, at /remote: no NabiMan menus, a slim
 * bar with the way back to NabiMan, the language, the theme and signing
 * out, and the whole rest of the window for the work. Signing in, idle
 * sign-out and settings are NabiMan's, shared.
 */
export default function RemoteShell({ username, allowed, onLogout }: Props) {
  const { t } = useT();
  const [folded, setFolded] = useChromeHidden();
  useEffect(() => {
    const before = document.title;
    document.title = t('remote.shell.title');
    return () => { document.title = before; };
  }, [t]);

  return (
    <div className={`remote-shell ${folded ? 'is-folded' : ''}`}>
      {folded ? (
        <button type="button" className="chrome-handle" onClick={() => setFolded(false)}
          title={t('app.showChrome')} aria-label={t('app.showChrome')}>▾</button>
      ) : (
        <header className="remote-shell-bar">
          <a className="remote-shell-brand" href="/" title={t('remote.shell.back')}>
            <img src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="20" height="20" />
            <span>NabiMan</span><strong>{t('remote.shell.name')}</strong>
          </a>
          <div className="remote-shell-actions">
            <span className="remote-shell-user">{username}</span>
            <ThemeSelector compact />
            <LanguageSelector />
            <a className="btn btn-secondary btn-sm" href="/">{t('remote.shell.back')}</a>
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setFolded(true)}
              title={t('app.hideChrome')} aria-label={t('app.hideChrome')}>▴</button>
            <button type="button" className="btn btn-secondary btn-sm" onClick={onLogout}>{t('app.logout')}</button>
          </div>
        </header>
      )}
      <main className="remote-shell-main">
        {allowed ? <RemoteWorkspace /> : (
          <div className="panel">
            <p role="alert">{t('remote.shell.notAllowed')}</p>
            <a className="btn btn-secondary" href="/">{t('remote.shell.back')}</a>
          </div>
        )}
      </main>
    </div>
  );
}

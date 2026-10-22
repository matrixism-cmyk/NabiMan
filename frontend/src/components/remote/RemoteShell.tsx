import React, { useEffect } from 'react';
import { useT } from '../../shared/i18n';
import { ThemeSelector } from '../../shared/theme';
import LanguageSelector from '../../shared/ui/LanguageSelector';
import RemoteWorkspace from './RemoteWorkspace';
import { MemberContext } from './member';
import { useChromeHidden } from '../../shared/ui/useChromeHidden';
import './remote-shell.css';

/** Whether this page was opened as the remote workspace on its own (/remote). */
export const isStandaloneRemote = () => /^\/remote\/?$/.test(window.location.pathname);

interface Props {
  username: string;
  /** Administrators only, as in NabiMan's own menu, and remote-only members. */
  allowed: boolean;
  /** A remote-only member: this is all of NabiMan they get, so no way back to it. */
  member?: boolean;
  onChangePassword?: () => void;
  onLogout: () => void;
}

/**
 * The remote workspace on its own, at /remote: no NabiMan menus, a slim
 * bar with the way back to NabiMan, the language, the theme and signing
 * out, and the whole rest of the window for the work. Signing in, idle
 * sign-out and settings are NabiMan's, shared.
 */
export default function RemoteShell({ username, allowed, member = false, onChangePassword, onLogout }: Props) {
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
          <a className="remote-shell-brand" href={member ? '/remote' : '/'} title={member ? undefined : t('remote.shell.back')}>
            <img src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="20" height="20" />
            <span>NabiMan</span><strong>{t('remote.shell.name')}</strong>
          </a>
          <div className="remote-shell-actions">
            <span className="remote-shell-user">{username}</span>
            <ThemeSelector compact />
            <LanguageSelector />
            {!member && <a className="btn btn-secondary btn-sm" href="/">{t('remote.shell.back')}</a>}
            {onChangePassword && (
              <button type="button" className="btn btn-secondary btn-sm" onClick={onChangePassword}>{t('app.changePassword')}</button>
            )}
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setFolded(true)}
              title={t('app.hideChrome')} aria-label={t('app.hideChrome')}>▴</button>
            <button type="button" className="btn btn-secondary btn-sm" onClick={onLogout}>{t('app.logout')}</button>
          </div>
        </header>
      )}
      <main className="remote-shell-main">
        {allowed ? <MemberContext.Provider value={member}><RemoteWorkspace /></MemberContext.Provider> : (
          <div className="panel">
            <p role="alert">{t('remote.shell.notAllowed')}</p>
            <a className="btn btn-secondary" href="/">{t('remote.shell.back')}</a>
          </div>
        )}
      </main>
    </div>
  );
}

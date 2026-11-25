import React, { useEffect, useMemo } from 'react';
import { useApi } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import { ErrorBoundary } from '../../shared/ui/ErrorBoundary';
import type { RemoteServer } from './remoteTypes';
import type { FileLink } from './fileLink';
import SftpPanel from './sftp/SftpPanel';
import { accountSource, baseName } from './sftp/sftpApi';
import './remote-shell.css';
import './remote-workspace.css';
import './sftp/sftp.css';

/**
 * What a link to a file opens (see ./fileLink): the server's file panel at
 * the file's folder, with the file shown over it. The panel asks the server
 * as it always does, so an account that may not open that server's files
 * sees the server's refusal instead.
 */
export default function FileLinkPage({ link }: { link: FileLink }) {
  const { t } = useT();
  const source = useMemo(() => accountSource(link.scope), [link.scope]);
  const name = baseName(link.path);
  const goTo = useMemo(() => {
    const at = link.path.replace(/\/+$/, '').lastIndexOf('/');
    return { path: at <= 0 ? '/' : link.path.slice(0, at), preview: name };
  }, [link.path, name]);
  const { data: servers } = useApi<RemoteServer[]>(link.scope === 'local' ? '' : '/api/remote-servers');
  const server = link.scope === 'local'
    ? t('remote.localShell')
    : servers?.find((s) => s.id === link.scope)?.name || link.scope;

  useEffect(() => {
    const before = document.title;
    document.title = `${name} · NabiMan`;
    // Another link pasted into this tab changes only the hash: start over with it.
    const reload = () => window.location.reload();
    window.addEventListener('hashchange', reload);
    return () => { document.title = before; window.removeEventListener('hashchange', reload); };
  }, [name]);

  return (
    <div className="remote-shell">
      <header className="remote-shell-bar">
        <a className="remote-shell-brand" href="/remote" title={t('remote.shell.back')}>
          <img src={`${process.env.PUBLIC_URL}/favicon.svg`} alt="" width="20" height="20" />
          <span>NabiMan</span><strong>{server}</strong>
        </a>
        <span className="file-link-path" title={link.path}>{link.path}</span>
      </header>
      <main className="remote-shell-main">
        <div className="rw-view-files file-link-files">
          <ErrorBoundary region="sftp" resetKey={source.base}>
            <SftpPanel source={source} serverName={server} goTo={goTo} />
          </ErrorBoundary>
        </div>
      </main>
    </div>
  );
}

import React, { useEffect, useState } from 'react';
import { useT } from '../../shared/i18n';
import VncConsoleView from './VncConsoleView';
import RdpConsoleView from './RdpConsoleView';
import { shareInfo, ShareInfo } from './vncApi';

/** The page a console share link opens (`/cshare/<token>`): no account, just the
 *  one console, view-only or control as the link was made. */
export default function ConsoleSharePage({ token }: { token: string }) {
  const { t } = useT();
  const [info, setInfo] = useState<ShareInfo | null>(null);
  const [gone, setGone] = useState(false);

  useEffect(() => {
    shareInfo(token).then((r) => { if (r.success && r.data) setInfo(r.data); else setGone(true); }).catch(() => setGone(true));
  }, [token]);
  useEffect(() => { if (info) document.title = `${info.label} · NabiMan`; }, [info]);

  if (gone) return <div className="rw-console-popout rw-console-missing">{t('remote.console.share.gone')}</div>;
  if (!info) return <div className="rw-console-popout rw-console-missing">{t('common.loading')}</div>;
  const share = { token, viewOnly: info.mode !== 'control' };
  return (
    <div className="rw-console-popout">
      {info.protocol === 'rdp'
        ? <RdpConsoleView target={token} label={info.label} popout share={share} />
        : <VncConsoleView vm={token} label={info.label} popout share={share} />}
    </div>
  );
}

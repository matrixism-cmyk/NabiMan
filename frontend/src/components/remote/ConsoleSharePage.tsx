import React, { useEffect, useState } from 'react';
import { useT } from '../../shared/i18n';
import VncConsoleView from './VncConsoleView';
import RdpConsoleView from './RdpConsoleView';
import { shareInfo, ShareInfo } from './vncApi';

/** The page a console share link opens (`/cshare/<token>`): no account, just the
 *  one console, view-only or control as the link was made, behind a password if
 *  one was set. */
export default function ConsoleSharePage({ token }: { token: string }) {
  const { t } = useT();
  const [info, setInfo] = useState<ShareInfo | null>(null);
  const [gone, setGone] = useState(false);
  const [pw, setPw] = useState('');
  const [entered, setEntered] = useState(false);

  useEffect(() => {
    shareInfo(token).then((r) => { if (r.success && r.data) setInfo(r.data); else setGone(true); }).catch(() => setGone(true));
  }, [token]);
  useEffect(() => { if (info) document.title = `${info.label} · NabiMan`; }, [info]);

  if (gone) return <div className="rw-console-popout rw-console-missing">{t('remote.console.share.gone')}</div>;
  if (!info) return <div className="rw-console-popout rw-console-missing">{t('common.loading')}</div>;

  if (info.needs_password && !entered) {
    return (
      <div className="rw-console-popout rw-console-missing">
        <form className="rw-section" style={{ maxWidth: 320 }} onSubmit={(e) => { e.preventDefault(); if (pw) setEntered(true); }}>
          <h3>{info.label}</h3>
          <p className="text-secondary" style={{ fontSize: 13 }}>{t('remote.console.share.passwordAsk')}</p>
          <input type="password" className="filter-input" autoFocus value={pw} onChange={(e) => setPw(e.target.value)} placeholder={t('remote.console.share.password')} />
          <button type="submit" className="btn btn-primary btn-sm" disabled={!pw} style={{ marginTop: 8 }}>{t('common.connect')}</button>
        </form>
      </div>
    );
  }

  const share = { token, viewOnly: info.mode !== 'control', pw: info.needs_password ? pw : undefined };
  return (
    <div className="rw-console-popout">
      {info.protocol === 'rdp'
        ? <RdpConsoleView target={token} label={info.label} popout share={share} />
        : <VncConsoleView vm={token} label={info.label} popout share={share} />}
    </div>
  );
}

import React, { useEffect } from 'react';
import { useApi } from '../../shared/api/useApi';
import { useT } from '../../shared/i18n';
import VncConsoleView from './VncConsoleView';
import RdpConsoleView from './RdpConsoleView';
import { VmConsole } from './vncApi';

/** A VM console detached into its own browser window (`/remote#console/<vm>`),
 *  shown full-window with no workspace chrome. Opened by the 분리 button. */
export const isConsolePopout = () =>
  /^\/remote\/?$/.test(window.location.pathname) && (window.location.hash.startsWith('#console/') || window.location.hash.startsWith('#rdp/'));

export default function ConsolePopout() {
  const { t } = useT();
  const isRdp = window.location.hash.startsWith('#rdp/');
  const vm = decodeURIComponent(window.location.hash.replace(/^#(console|rdp)\//, ''));
  const { data: consoles } = useApi<VmConsole[]>('/api/remote/consoles');
  const found = (consoles || []).find((c) => c.name === vm);
  const label = found?.label || vm;

  useEffect(() => { document.title = `${label} · NabiMan`; }, [label]);

  if (consoles && !found) {
    return <div className="rw-console-popout rw-console-missing">{t('remote.console.notFound')}</div>;
  }
  return (
    <div className="rw-console-popout">
      {isRdp || found?.protocol === 'rdp'
        ? <RdpConsoleView target={vm} label={label} popout />
        : <VncConsoleView vm={vm} label={label} popout />}
    </div>
  );
}

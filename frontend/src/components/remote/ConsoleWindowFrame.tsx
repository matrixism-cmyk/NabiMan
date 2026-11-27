import React, { useState } from 'react';
import { useT } from '../../shared/i18n';
import { ErrorBoundary } from '../../shared/ui/ErrorBoundary';
import WindowShell, { ShellHandlers } from '../terminal/WindowShell';
import type { ConsoleWin } from '../terminal/windowGeometry';
import { popOutConsole } from './consoleHandOver';
import ConsoleShareModal from './ConsoleShareModal';
import { ConsoleSuspense, RdpConsoleView, VncConsoleView } from './lazyConsoles';

interface Props extends ShellHandlers {
  win: ConsoleWin;
  /** The top window of those that show: only then do keys go to the VM. */
  focused: boolean;
  /** Whether the consoles the account may open are known (a window from a saved layout waits for that). */
  ready: boolean;
  onClose: (id: string) => void;
}

/**
 * A VM console as a floating window, in the same frame as a terminal window.
 * The view inside is the workspace's own, made compact; the connection is the
 * console keeper's, so closing the window only puts the console out of sight
 * (the view's "연결 끊기" ends it), and a minimised window — still mounted,
 * off the page — keeps it without counting as out of sight.
 */
export default function ConsoleWindowFrame({ win, focused, ready, onUpdate, onFocus, onClose }: Props) {
  const { t } = useT();
  const c = win.console;
  const [sharing, setSharing] = useState(false);
  const common = { label: c.label, windowed: true, focused: focused && !win.minimized, canShare: true, onShare: () => setSharing(true) };
  return (
    <WindowShell
      win={win}
      onUpdate={onUpdate}
      onFocus={onFocus}
      className="term-window-console"
      lead={<>
        <span className="term-kind-tag" aria-label={t('terminal.consoleWindow')}>VM</span>
        <span className="term-window-title">{c.label}</span>
        <span className="term-window-sub">{c.protocol.toUpperCase()}</span>
      </>}
      actions={
        // The browser window connects for itself; this tab hangs up first and the window here goes.
        <button className="term-window-popout" title={t('remote.console.popOutHint')}
          onClick={(e) => { e.stopPropagation(); popOutConsole(c); }}>⧉ {t('remote.console.popOut')}</button>
      }
      closing={
        <button className="term-window-close" title={t('terminal.closeConsoleHint')} aria-label={t('terminal.closeConsoleHint')}
          onClick={(e) => { e.stopPropagation(); onClose(win.id); }}>✕</button>
      }
      extras={sharing && <ConsoleShareModal console={c.name} label={c.label} onClose={() => setSharing(false)} />}
    >
      {ready ? (
        <ErrorBoundary region="console" resetKey={c.name}>
          <ConsoleSuspense>
            {c.protocol === 'rdp'
              ? <RdpConsoleView key={c.name} target={c.name} {...common} />
              : <VncConsoleView key={c.name} vm={c.name} {...common} />}
          </ConsoleSuspense>
        </ErrorBoundary>
      ) : <div className="rw-view rw-console-missing" role="status">{t('remote.console.loading')}</div>}
    </WindowShell>
  );
}

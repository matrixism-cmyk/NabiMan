import React from 'react';
import { useT } from '../../shared/i18n';
import type { ConsoleShot } from '../remote/consoleSnapshots';
import type { VmConsole } from '../remote/vncApi';

/** How many of a screen's last lines a thumbnail shows. */
export const THUMB_LINES = 8;

/** What an account needs of a session to tell it from the others. */
export interface StartSession { id: string; label: string; title: string; alive: boolean; busy?: boolean; agents?: string[]; preview?: string }

/** The last lines on a session's screen, without the blank ones under them. */
export function thumbLines(preview = '', count = THUMB_LINES): string[] {
  const lines = preview.replace(/\s+$/, '').split('\n').map(line => line.replace(/\s+$/, ''));
  return lines.some(Boolean) ? lines.slice(-count) : [];
}

/** A session's screen as tiny text: enough to see which one it is. */
export function ScreenText({ preview }: { preview?: string }) {
  const { t } = useT();
  const lines = thumbLines(preview);
  if (lines.length === 0) return <span className="term-start-screen is-empty" aria-hidden="true">{t('terminal.noPreview')}</span>;
  return <pre className="term-start-screen" aria-hidden="true">{lines.join('\n')}</pre>;
}

/** One running session: its screen, its name, where it runs and what is going on in it. */
export function SessionCard({ session, name, sub, windowed, onOpen }: {
  session: StartSession; name: string; sub: string; windowed: boolean; onOpen: () => void;
}) {
  const { t } = useT();
  const agents = session.agents || [];
  return (
    <button type="button" role="menuitem" className="term-start-card" onClick={onOpen} title={session.label}>
      <ScreenText preview={session.preview} />
      <span className="term-start-line">
        <span className={`term-start-dot ${session.busy ? 'is-busy' : ''}`} />
        <span className="term-start-name">{name}</span>
      </span>
      {(sub || agents.length > 0 || windowed) && (
        <span className="term-start-line">
          <span className="term-start-sub">{sub}</span>
          {agents.length > 0 && <span className="term-start-tag">{agents.join(' · ')}</span>}
          {windowed && <span className="term-start-tag is-open">{t('start.windowed')}</span>}
        </span>
      )}
    </button>
  );
}

/** How long ago, in the words of the menu. */
export function agoText(t: (key: string, params?: Record<string, string | number>) => string, at: number, now = Date.now()): string {
  const minutes = Math.floor(Math.max(0, now - at) / 60000);
  if (minutes < 1) return t('start.justNow');
  return minutes < 60 ? t('start.minutesAgo', { n: minutes }) : t('start.hoursAgo', { n: Math.floor(minutes / 60) });
}

/**
 * One VM console. Its picture is the screen this tab already has open (or had:
 * then dimmed, and said to be the screen last seen); a console never opened
 * here shows only how it is reached.
 */
export function ConsoleCard({ console: c, shot, fresh, held, windowed, onOpen, onPopOut }: {
  console: VmConsole; shot?: ConsoleShot; fresh: boolean; held: boolean;
  /** It has a window on this page already: opening raises that one. */
  windowed?: boolean;
  onOpen: () => void;
  /** "새창으로 분리": a browser window instead. Shift+click on the card does the same. */
  onPopOut?: () => void;
}) {
  const { t } = useT();
  const protocol = c.protocol === 'rdp' ? 'RDP' : 'VNC';
  return (
    <span className="term-start-cardwrap">
      <button type="button" role="menuitem" className="term-start-card" title={t('start.consoleHint', { label: c.label })}
        onClick={(e) => (e.shiftKey && onPopOut ? onPopOut() : onOpen())}>
        <span className={`term-start-shot ${shot && !fresh ? 'is-stale' : ''}`}>
          {shot ? <><img src={shot.url} alt="" draggable={false} /><span className="term-start-tag">{protocol}</span></> : protocol}
        </span>
        <span className="term-start-line"><span className="term-start-name">{c.label}</span></span>
        {(held || windowed || (shot && !fresh)) && (
          <span className="term-start-line">
            {shot && !fresh && <span className="term-start-sub">{t('start.lastSeen', { ago: agoText(t, shot.at) })}</span>}
            {windowed ? <span className="term-start-tag is-open">{t('start.windowed')}</span>
              : held && <span className="term-start-tag is-open">{t('remote.console.kept')}</span>}
          </span>
        )}
      </button>
      {onPopOut && (
        <button type="button" role="menuitem" className="term-start-popout" onClick={onPopOut}
          title={`${t('remote.console.popOut')} — ${t('remote.console.popOutHint')}`}
          aria-label={t('start.consolePopOut', { label: c.label })}>⇱</button>
      )}
    </span>
  );
}

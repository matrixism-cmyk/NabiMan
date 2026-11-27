import React from 'react';
import { useT } from '../../shared/i18n';
import type { VmConsole } from '../remote/vncApi';
import StartMenu from './StartMenu';
import type { TerminalTarget } from './TerminalView';
import type { TermWindow } from './windowGeometry';

interface Props {
  windows: TermWindow[];
  onFocus: (id: string) => void;
  onTile: () => void;
  onCascade: () => void;
  onOpenSettings: () => void;
  onOpen: (target: TerminalTarget, options?: { title?: string; joinSessionId?: string }) => void;
  onOpenConsole: (c: VmConsole) => void;
  onToggleAll: () => void;
}

/** The bar under the windows: the start button, one item for every window —
 *  terminal or VM console — and what is done to them all at once. */
export default function TerminalTaskbar({ windows, onFocus, onTile, onCascade, onOpenSettings, onOpen, onOpenConsole, onToggleAll }: Props) {
  const { t } = useT();
  const showing = windows.some((w) => !w.minimized);
  return (
    <div className="term-taskbar">
      <StartMenu windows={windows} open={onOpen} openConsole={onOpenConsole} />
      {windows.map((win) => (
        <button
          key={win.id}
          className={`term-taskbar-item ${win.minimized ? 'minimized' : ''} ${win.kind === 'console' ? 'is-console' : ''}`}
          onClick={() => onFocus(win.id)}
          title={win.kind === 'console' ? `${t('terminal.consoleWindow')} · ${win.subtitle}` : win.subtitle}
        >
          {win.minimized ? '▣ ' : '▪ '}
          {/* Told from a terminal by the word, not by colour. */}
          {win.kind === 'console' && <span className="term-kind-tag" aria-label={t('terminal.consoleWindow')}>VM</span>}
          {win.title}{win.kind === 'terminal' && win.sessionName ? ` - ${win.sessionName}` : ''}
        </button>
      ))}
      <span className="term-taskbar-spacer" />
      {/* With no window open the bar is only the start button and the settings. */}
      {windows.length > 0 && (
        <button className="term-taskbar-btn" onClick={onToggleAll} aria-pressed={!showing}>
          {showing ? `▁ ${t('terminal.hideAll')}` : `▣ ${t('terminal.showAll')}`}
        </button>
      )}
      {windows.length > 0 && <button className="term-taskbar-btn" onClick={onTile} title={t('terminal.tile')}>⊞ {t('terminal.tile')}</button>}
      {windows.length > 0 && <button className="term-taskbar-btn" onClick={onCascade} title={t('terminal.cascade')}>⧉ {t('terminal.cascade')}</button>}
      <button className="term-taskbar-btn" onClick={onOpenSettings} title={t('terminal.settings')}>⚙ {t('common.settings')}</button>
    </div>
  );
}

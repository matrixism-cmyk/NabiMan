import React, { useCallback, useEffect, useRef } from 'react';
import type { Terminal } from 'xterm';
import TerminalHistoryBar, { HistoryPosition } from './TerminalHistoryBar';
import { useT } from '../../i18n';

interface Props {
  screenId: string;
  position: HistoryPosition | null;
  connected: boolean;
  termRef: React.RefObject<Terminal | null>;
  wsRef: React.RefObject<WebSocket | null>;
}

export default function TerminalHistoryControls({ screenId, position, connected, termRef, wsRef }: Props) {
  const historyRef = useRef<HTMLDivElement>(null);
  const { t } = useT();
  const scrollHistory = useCallback((direction: number) => {
    const term = termRef.current;
    if (!term) return;
    if (term.buffer.active.type === 'normal') {
      term.scrollLines(direction * 3);
      return;
    }
    // tmux owns alternate-screen history. Send the same mouse wheel event
    // as scrolling over its text, rather than moving xterm's empty viewport.
    const screen = term.element?.querySelector('.xterm-screen');
    if (!screen) return;
    const rect = screen.getBoundingClientRect();
    screen.dispatchEvent(new WheelEvent('wheel', {
      bubbles: true, cancelable: true, deltaY: direction * 120,
      clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2,
    }));
  }, [termRef]);

  useEffect(() => {
    const history = historyRef.current;
    if (!history) return;
    const wheel = (event: WheelEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.deltaY) scrollHistory(Math.sign(event.deltaY));
    };
    history.addEventListener('wheel', wheel, { passive: false });
    return () => history.removeEventListener('wheel', wheel);
  }, [scrollHistory]);

  return (
    <div ref={historyRef} className="terminal-history-controls" title={t('terminal.historyHint')}>
      <button type="button" disabled={!connected} aria-label={t('terminal.historyUp')}
        title={t('terminal.historyUp')} onClick={() => scrollHistory(-1)}>▲</button>
      <TerminalHistoryBar controlsId={screenId} position={position} connected={connected} onSeek={offset => {
        if (position && wsRef.current?.readyState === WebSocket.OPEN) {
          wsRef.current.send(`\x01HISTORY:${offset}`);
        }
      }} />
      <button type="button" disabled={!connected} aria-label={t('terminal.historyDown')}
        title={t('terminal.historyDown')} onClick={() => scrollHistory(1)}>▼</button>
    </div>
  );
}

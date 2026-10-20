import React, { useCallback, useEffect, useRef } from 'react';
import type { Terminal } from 'xterm';
import TerminalHistoryBar, { HistoryPosition } from './TerminalHistoryBar';
import { useT } from '../../i18n';
import { scrollTerminalHistory } from './touchGestures';

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
    // tmux owns alternate-screen history: it gets the wheel event a mouse
    // over its text would send, rather than xterm's empty viewport moving.
    if (termRef.current) scrollTerminalHistory(termRef.current, direction);
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

import React, { useMemo, useRef, useState } from 'react';
import { useT } from '../../shared/i18n';
import SftpPanel from './sftp/SftpPanel';
import { draggingOwnRows } from './sftp/sftpDrop';
import { accountSource, SftpSource } from './sftp/sftpApi';
import type { TerminalTarget } from '../terminal/terminalTypes';
import './sftp/sftp.css';

interface Props {
  /** What the terminal is connected to: a saved server or the local shell
   *  have files to show; an unsaved SSH target does not. */
  target: TerminalTarget | null;
  /** Shown as the panel's accessible name. */
  name: string;
  /** The workspace pane opens the panel; a floating window starts folded. */
  place?: 'pane' | 'window';
  /** Files from somewhere other than the target's own: a share link's
   *  visitor. `null` shows no panel. */
  source?: SftpSource | null;
  /** Types text into the terminal. */
  onInsert: (text: string) => void;
  /** Whether the panel is open, when the caller decides (a phone opens it
   *  from its header, over the terminal, and shows no side tab). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  children: React.ReactNode;
}

/** The file panel's scope for a terminal target: a saved server id or `local`. */
export function filesScope(target: TerminalTarget | null): string | null {
  if (!target) return null;
  if (target.kind === 'local') return 'local';
  return target.noFiles ? null : target.serverId || null;
}
const MIN_WIDTH = 240;
const MIN_TERMINAL = 320;

const read = (key: string) => { try { return localStorage.getItem(key); } catch { return null; } };
const write = (key: string, value: string) => { try { localStorage.setItem(key, value); } catch { /* private mode */ } };

/**
 * A terminal with the server's files beside it. The panel can be closed to a
 * thin tab and resized by dragging the edge between the two; both are
 * remembered. Rows dragged out of the panel onto the terminal type their
 * paths at the prompt.
 */
export default function TerminalWithFiles({
  target, name, place = 'pane', source: given, onInsert, open: openGiven, onOpenChange, children,
}: Props) {
  const { t } = useT();
  const OPEN_KEY = place === 'pane' ? 'nabiman_sftp_open' : 'nabiman_sftp_open_window';
  const WIDTH_KEY = place === 'pane' ? 'nabiman_sftp_width' : 'nabiman_sftp_width_window';
  const [openOwn, setOpen] = useState(() => (place === 'pane' ? read(OPEN_KEY) !== '0' : read(OPEN_KEY) === '1'));
  const controlled = onOpenChange !== undefined;
  const open = controlled ? Boolean(openGiven) : openOwn;
  const [width, setWidth] = useState(() => Number(read(WIDTH_KEY)) || (place === 'pane' ? 360 : 300));
  const [dropping, setDropping] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const scope = filesScope(target);
  const own = useMemo(() => (scope ? accountSource(scope) : null), [scope]);
  const source = given === undefined ? own : given;
  if (!source) return <>{children}</>;

  const toggle = (next: boolean) => {
    if (controlled) { onOpenChange(next); return; }
    setOpen(next);
    write(OPEN_KEY, next ? '1' : '0');
  };

  const startResize = (e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault();
    const handle = e.currentTarget;
    handle.setPointerCapture(e.pointerId);
    const total = rootRef.current?.clientWidth || window.innerWidth;
    const right = rootRef.current?.getBoundingClientRect().right || window.innerWidth;
    let latest = width;
    const move = (ev: PointerEvent) => {
      latest = Math.round(Math.max(MIN_WIDTH, Math.min(total - MIN_TERMINAL, right - ev.clientX)));
      setWidth(latest);
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
      write(WIDTH_KEY, String(latest));
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
  };

  const resizeByKey = (e: React.KeyboardEvent) => {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    const next = Math.max(MIN_WIDTH, width + (e.key === 'ArrowLeft' ? 24 : -24));
    setWidth(next);
    write(WIDTH_KEY, String(next));
  };

  return (
    <div className={`rw-split rw-split-${place}`} ref={rootRef}>
      <div
        className={`rw-split-term ${dropping ? 'is-drop' : ''}`}
        onDragOver={(e) => {
          if (!draggingOwnRows(e.dataTransfer)) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = 'copy';
          setDropping(true);
        }}
        onDragLeave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropping(false); }}
        onDrop={(e) => {
          if (!draggingOwnRows(e.dataTransfer)) return;
          e.preventDefault();
          setDropping(false);
          const text = e.dataTransfer.getData('text/plain');
          if (text) onInsert(`${text} `);
        }}
      >
        {children}
        {dropping && <div className="rw-split-drop" aria-hidden="true">{t('sftp.dropOnTerminal')}</div>}
      </div>
      {open ? (
        <>
          <div className="rw-splitter" role="separator" aria-orientation="vertical" tabIndex={0}
            aria-label={t('sftp.resize')} aria-valuenow={width}
            onPointerDown={startResize} onKeyDown={resizeByKey} onDoubleClick={() => toggle(false)} />
          <div className="rw-split-files" style={{ width }}>
            <SftpPanel source={source} serverName={name} onInsert={onInsert} onClose={() => toggle(false)} />
          </div>
        </>
      ) : controlled ? null : (
        <button type="button" className="rw-split-tab" onClick={() => toggle(true)}
          title={t('sftp.openHint')} aria-label={t('sftp.show')}>
          <span className="rw-split-tab-arrow" aria-hidden="true">«</span>
          <span aria-hidden="true">📁</span> {t('sftp.tab')}
        </button>
      )}
    </div>
  );
}

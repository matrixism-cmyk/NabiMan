import React, { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { useT } from '../../../shared/i18n';
import { Columns, ColumnKey, OPTIONAL } from './sftpColumns';

interface Props {
  x: number;
  y: number;
  columns: Columns;
  label: (key: ColumnKey) => string;
  opener?: HTMLElement | null;
  onClose: () => void;
}

const EDGE = 8;

/**
 * The right-click "표시 가능 열" panel: a checkbox per column to show or hide
 * it, and ▲▼ on the right to move a shown column up or down in the list.
 * Shown columns come first, in their display order; the rest follow.
 */
export default function ColumnChooser({ x, y, columns, label, opener, onClose }: Props) {
  const { t } = useT();
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: x, top: y });
  const { shown } = columns.state;
  // Shown columns in order, then the ones still available to add.
  const ordered = [...shown, ...OPTIONAL.filter((k) => !shown.includes(k))];

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const left = x + width + EDGE > window.innerWidth ? Math.max(EDGE, x - width) : x;
    const top = y + height + EDGE > window.innerHeight ? Math.max(EDGE, window.innerHeight - height - EDGE) : y;
    setPos({ left, top });
  }, [x, y, shown.length]);

  useEffect(() => {
    const onDown = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) onClose(); };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') { onClose(); opener?.focus(); } };
    document.addEventListener('mousedown', onDown, true);
    document.addEventListener('keydown', onKey, true);
    window.addEventListener('resize', onClose);
    return () => {
      document.removeEventListener('mousedown', onDown, true);
      document.removeEventListener('keydown', onKey, true);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose, opener]);

  return (
    <div ref={ref} className="sftp-colmenu" role="dialog" aria-label={t('sftp.col.available')}
      style={{ left: pos.left, top: pos.top }} onClick={(e) => e.stopPropagation()}>
      <div className="sftp-colmenu-title">{t('sftp.col.available')}</div>
      <div className="sftp-colmenu-hint">{t('sftp.col.choosePrompt')}</div>
      <ul className="sftp-colmenu-list">
        {ordered.map((key) => {
          const on = shown.includes(key);
          const i = shown.indexOf(key);
          return (
            <li key={key} className={`sftp-colmenu-item ${on ? 'is-on' : ''}`}>
              <label className="sftp-colmenu-check">
                <input type="checkbox" checked={on} onChange={() => columns.toggle(key)} />
                <span>{label(key)}</span>
              </label>
              <span className="sftp-colmenu-move">
                <button type="button" aria-label={t('sftp.col.moveUp', { name: label(key) })} title={t('sftp.col.moveUp', { name: label(key) })}
                  disabled={!on || i <= 0} onClick={() => columns.move(key, -1)}>▲</button>
                <button type="button" aria-label={t('sftp.col.moveDown', { name: label(key) })} title={t('sftp.col.moveDown', { name: label(key) })}
                  disabled={!on || i < 0 || i >= shown.length - 1} onClick={() => columns.move(key, 1)}>▼</button>
              </span>
            </li>
          );
        })}
      </ul>
      <button type="button" className="sftp-colmenu-reset" onClick={() => { columns.reset(); }}>↺ {t('sftp.col.reset')}</button>
    </div>
  );
}

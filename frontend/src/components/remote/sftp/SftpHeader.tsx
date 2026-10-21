import React, { RefObject, useRef, useState } from 'react';
import { useT } from '../../../i18n';
import RailContextMenu, { RailMenuItem } from '../RailContextMenu';
import { ColumnKey, Columns, MAX_WIDTH, MIN_WIDTH, OPTIONAL } from './sftpColumns';

interface Props {
  columns: Columns;
  /** The list, to measure its cells when a column is fitted to them. */
  listRef: RefObject<HTMLElement | null>;
}

const LABEL: Record<ColumnKey, string> = {
  name: 'sftp.col.name', size: 'sftp.col.size', mtime: 'sftp.col.mtime', mode: 'sftp.col.mode', owner: 'sftp.col.owner', group: 'sftp.col.group',
};

/**
 * The file list's headings: a click sorts by that column (again, the other
 * way), a right-click chooses which columns show, dragging a heading's right
 * edge sets its width, and a double-click there fits it to what it holds.
 */
export default function SftpHeader({ columns, listRef }: Props) {
  const { t } = useT();
  const { state } = columns;
  const [menu, setMenu] = useState<{ x: number; y: number; opener: HTMLElement } | null>(null);
  const dragged = useRef(false);

  const fit = (key: ColumnKey, cell: HTMLElement) => {
    const cells = Array.from(listRef.current?.querySelectorAll<HTMLElement>(`.sftp-c-${key}`) || []);
    const label = cell.querySelector<HTMLElement>('.sftp-colhead-label');
    const widest = Math.max(label ? label.scrollWidth + 18 : 0, ...cells.map((c) => c.scrollWidth));
    columns.setWidth(key, Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, widest + 10)));
  };

  const resize = (key: ColumnKey) => (e: React.PointerEvent<HTMLSpanElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const handle = e.currentTarget;
    const cell = handle.parentElement as HTMLElement;
    const start = { x: e.clientX, width: cell.getBoundingClientRect().width };
    handle.setPointerCapture(e.pointerId);
    dragged.current = false;
    const move = (ev: PointerEvent) => {
      if (Math.abs(ev.clientX - start.x) > 2) dragged.current = true;
      columns.setWidth(key, start.width + ev.clientX - start.x);
    };
    const up = () => {
      handle.removeEventListener('pointermove', move);
      handle.removeEventListener('pointerup', up);
      handle.removeEventListener('pointercancel', up);
    };
    handle.addEventListener('pointermove', move);
    handle.addEventListener('pointerup', up);
    handle.addEventListener('pointercancel', up);
  };

  const heading = (key: ColumnKey) => {
    const sorted = state.sort.key === key;
    return (
      <div key={key} className={`sftp-colhead-cell sftp-ch-${key}`} role="columnheader"
        aria-sort={sorted ? (state.sort.desc ? 'descending' : 'ascending') : 'none'}>
        <button type="button" className="sftp-colhead-btn" title={t('sftp.col.sortHint')}
          onClick={() => { if (!dragged.current) columns.sortBy(key); dragged.current = false; }}>
          <span className="sftp-colhead-label">{t(LABEL[key])}</span>
          {sorted && <span className="sftp-colhead-arrow" aria-hidden="true">{state.sort.desc ? '▼' : '▲'}</span>}
        </button>
        <span className="sftp-colhead-grip" role="separator" aria-orientation="vertical" aria-label={t('sftp.col.resize', { name: t(LABEL[key]) })}
          title={t('sftp.col.resizeHint')} onPointerDown={resize(key)}
          onClick={(e) => e.stopPropagation()}
          onDoubleClick={(e) => { e.stopPropagation(); fit(key, e.currentTarget.parentElement as HTMLElement); }} />
      </div>
    );
  };

  const items: RailMenuItem[] = [
    ...OPTIONAL.map((key) => ({
      key, icon: state.shown.includes(key) ? '✓' : '', label: t(LABEL[key]), onSelect: () => columns.toggle(key),
    })),
    { key: 'reset', icon: '↺', label: t('sftp.col.reset'), separated: true, onSelect: columns.reset },
  ];

  return (
    <div className="sftp-row sftp-colhead" role="row"
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => { e.preventDefault(); e.stopPropagation(); setMenu({ x: e.clientX, y: e.clientY, opener: e.currentTarget }); }}>
      <span aria-hidden="true" />
      {heading('name')}
      {state.shown.map((key) => heading(key))}
      {menu && (
        <RailContextMenu x={menu.x} y={menu.y} opener={menu.opener} title={t('sftp.col.choose')} items={items} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}

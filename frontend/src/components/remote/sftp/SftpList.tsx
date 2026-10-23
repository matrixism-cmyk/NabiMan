import React, { useEffect, useRef, useState } from 'react';
import { useT } from '../../../shared/i18n';
import { useLongPress } from '../../../shared/ui/useLongPress';
import { formatSize, isFolder, modeString, SftpEntry } from './sftpApi';
import SftpHeader from './SftpHeader';
import { ColumnKey, Columns, fileType, minWidth, ownerGroup, template } from './sftpColumns';

export interface Editing {
  mode: 'new' | 'rename';
  /** The entry being renamed; empty for a new folder. */
  from: string;
}

interface Props {
  entries: SftpEntry[];
  /** Which columns show, how wide, sorted how. */
  columns: Columns;
  /** Shown as a ".." row: open it or drop onto it to go one folder up. */
  parent: string | null;
  selected: Set<string>;
  dropOn: string | null;
  editing: Editing | null;
  pathOf: (name: string) => string;
  onClick: (e: React.MouseEvent, entry: SftpEntry) => void;
  onOpen: (entry: SftpEntry) => void;
  onUp: () => void;
  onMenu: (e: React.MouseEvent, entry: SftpEntry) => void;
  onDragStart: (e: React.DragEvent, entry: SftpEntry) => void;
  onDragEnd: (entry: SftpEntry) => void;
  /** The pointer rests on, or presses, a row that may be dragged out next. */
  onPrefetch: (entry: SftpEntry, now: boolean) => void;
  /** Drag over a folder row: the row's own path is the drop target. */
  onDragOverDir: (e: React.DragEvent, dir: string) => void;
  onDropDir: (e: React.DragEvent, dir: string) => void;
  onEditDone: (name: string | null) => void;
}

const ICONS: Record<SftpEntry['kind'], string> = { dir: '📁', file: '📄', link: '🔗', other: '⚙' };

function EditRow({ initial, onDone }: { initial: string; onDone: (name: string | null) => void }) {
  const ref = useRef<HTMLInputElement>(null);
  // Enter ends the edit, and the blur that follows must not end it again.
  const ended = useRef(false);
  const finish = (name: string | null) => {
    if (ended.current) return;
    ended.current = true;
    onDone(name);
  };
  useEffect(() => {
    const input = ref.current;
    if (!input) return;
    input.focus();
    // Select the name without its extension, as file managers do.
    const dot = initial.lastIndexOf('.');
    input.setSelectionRange(0, dot > 0 ? dot : initial.length);
  }, [initial]);
  return (
    <input
      ref={ref}
      className="sftp-edit"
      defaultValue={initial}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') finish(e.currentTarget.value.trim() || null);
        if (e.key === 'Escape') finish(null);
      }}
      onBlur={(e) => finish(e.currentTarget.value.trim() || null)}
    />
  );
}

function when(mtime: number | null): string {
  if (!mtime) return '';
  const date = new Date(mtime * 1000);
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return date.toLocaleString(undefined, sameYear
    ? { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }
    : { year: 'numeric', month: '2-digit', day: '2-digit' });
}

/** The row's tooltip: name, permissions and owner, where they are known. */
function describe(e: SftpEntry): string {
  const parts = [e.name];
  if (e.mode !== null) parts.push(`${modeString(e.mode)} (${e.mode.toString(8).padStart(4, '0')})`);
  const owner = e.owner ?? e.uid, group = e.group ?? e.gid;
  if (owner !== undefined || group !== undefined) parts.push(`${owner ?? '?'}:${group ?? '?'}`);
  return parts.join('  ·  ');
}

const coarsePointer = () => typeof window.matchMedia === 'function' && window.matchMedia('(pointer: coarse)').matches;

/** A click as if Ctrl were held: what a tap means while picking several. */
const adding = (e: React.MouseEvent) => ({ ...e, ctrlKey: true, metaKey: false, shiftKey: false }) as React.MouseEvent;

export default function SftpList(props: Props) {
  const { t } = useT();
  const { entries, parent, selected, dropOn, editing, pathOf, columns } = props;
  const tableRef = useRef<HTMLDivElement>(null);
  const cell = (entry: SftpEntry, key: Exclude<ColumnKey, 'name'>) => {
    const folder = isFolder(entry);
    let value: string;
    if (key === 'size') value = folder ? '' : formatSize(entry.size);
    else if (key === 'mtime') value = when(entry.mtime);
    else if (key === 'mode') value = entry.mode !== null ? modeString(entry.mode) : '';
    else if (key === 'ownergroup') value = ownerGroup(entry);
    else { const ft = fileType(entry); value = ft.label ? t(ft.label) : ft.ext.toUpperCase(); }
    return <span key={key} className={`sftp-col sftp-c-${key}`}>{value}</span>;
  };
  // On a touch screen a tap opens, as phones do; a long press opens the menu
  // and starts picking, after which taps add and remove rows until none is left.
  const [picking, setPicking] = useState(false);
  useEffect(() => { if (!selected.size) setPicking(false); }, [selected.size]);
  const press = useLongPress<SftpEntry | null>((at, entry) => {
    if (!entry) return;
    setPicking(true);
    props.onMenu({ preventDefault() {}, stopPropagation() {}, clientX: at.x, clientY: at.y, currentTarget: at.el } as unknown as React.MouseEvent, entry);
  });
  const tap = (e: React.MouseEvent, entry: SftpEntry) => {
    if (press.wasLong()) return;
    if (!press.wasTouch()) { props.onClick(e, entry); return; }
    if (picking) props.onClick(adding(e), entry);
    else props.onOpen(entry);
  };

  return (
    <div className="sftp-table" ref={tableRef} style={{ '--sftp-cols': template(columns.state), minWidth: `max(100%, ${minWidth(columns.state)}px)` } as React.CSSProperties}>
    <SftpHeader columns={columns} listRef={tableRef} />
    <div className="sftp-rows" role="listbox" aria-multiselectable="true" aria-label={t('sftp.title')}>
      {parent && (
        <div
          className={`sftp-row is-up ${dropOn === parent ? 'is-drop' : ''}`}
          onDoubleClick={props.onUp}
          {...press.bind(null)}
          onClick={() => { if (press.wasTouch()) props.onUp(); }}
          onDragOver={(e) => props.onDragOverDir(e, parent)}
          onDrop={(e) => props.onDropDir(e, parent)}
          title={t('sftp.up')}
        >
          <span className="sftp-icon" aria-hidden="true">↰</span>
          <span className="sftp-name">..</span>
        </div>
      )}
      {editing?.mode === 'new' && (
        <div className="sftp-row is-editing">
          <span className="sftp-icon" aria-hidden="true">📁</span>
          <EditRow initial="" onDone={props.onEditDone} />
        </div>
      )}
      {entries.map((entry) => {
        const folder = isFolder(entry);
        const path = pathOf(entry.name);
        const renaming = editing?.mode === 'rename' && editing.from === entry.name;
        return (
          <div
            key={entry.name}
            role="option"
            aria-selected={selected.has(entry.name)}
            className={[
              'sftp-row',
              selected.has(entry.name) ? 'is-selected' : '',
              folder ? 'is-folder' : '',
              dropOn === path ? 'is-drop' : '',
            ].join(' ')}
            // A finger's long press opens the menu; dragging rows out is for a mouse.
            draggable={!renaming && !coarsePointer()}
            {...press.bind(entry)}
            onClick={(e) => tap(e, entry)}
            onDoubleClick={() => { if (!press.wasTouch()) props.onOpen(entry); }}
            onContextMenu={(e) => { if (press.justFired()) e.preventDefault(); else props.onMenu(e, entry); }}
            onDragStart={(e) => props.onDragStart(e, entry)}
            onDragEnd={() => props.onDragEnd(entry)}
            onPointerEnter={() => props.onPrefetch(entry, false)}
            onPointerDown={(e) => { press.bind(entry).onPointerDown(e); props.onPrefetch(entry, true); }}
            onDragOver={folder ? (e) => props.onDragOverDir(e, path) : undefined}
            onDrop={folder ? (e) => props.onDropDir(e, path) : undefined}
            title={describe(entry)}
          >
            <span className="sftp-icon" aria-hidden="true">{entry.to_dir ? '📁' : ICONS[entry.kind]}</span>
            {renaming
              ? <EditRow initial={entry.name} onDone={props.onEditDone} />
              : <span className="sftp-name sftp-c-name">{entry.name}</span>}
            {columns.state.shown.map((key) => cell(entry, key))}
          </div>
        );
      })}
    </div>
    </div>
  );
}

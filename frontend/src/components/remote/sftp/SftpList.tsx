import React, { useEffect, useRef } from 'react';
import { useT } from '../../../i18n';
import { formatSize, isFolder, SftpEntry } from './sftpApi';

export interface Editing {
  mode: 'new' | 'rename';
  /** The entry being renamed; empty for a new folder. */
  from: string;
}

interface Props {
  entries: SftpEntry[];
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

export default function SftpList(props: Props) {
  const { t } = useT();
  const { entries, parent, selected, dropOn, editing, pathOf } = props;

  return (
    <div className="sftp-rows" role="listbox" aria-multiselectable="true" aria-label={t('sftp.title')}>
      {parent && (
        <div
          className={`sftp-row is-up ${dropOn === parent ? 'is-drop' : ''}`}
          onDoubleClick={props.onUp}
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
            draggable={!renaming}
            onClick={(e) => props.onClick(e, entry)}
            onDoubleClick={() => props.onOpen(entry)}
            onContextMenu={(e) => props.onMenu(e, entry)}
            onDragStart={(e) => props.onDragStart(e, entry)}
            onDragEnd={() => props.onDragEnd(entry)}
            onPointerEnter={() => props.onPrefetch(entry, false)}
            onPointerDown={() => props.onPrefetch(entry, true)}
            onDragOver={folder ? (e) => props.onDragOverDir(e, path) : undefined}
            onDrop={folder ? (e) => props.onDropDir(e, path) : undefined}
            title={entry.mode !== null ? `${entry.name}  (${entry.mode.toString(8).padStart(4, '0')})` : entry.name}
          >
            <span className="sftp-icon" aria-hidden="true">{entry.to_dir ? '📁' : ICONS[entry.kind]}</span>
            {renaming
              ? <EditRow initial={entry.name} onDone={props.onEditDone} />
              : <span className="sftp-name">{entry.name}</span>}
            <span className="sftp-size">{folder ? '' : formatSize(entry.size)}</span>
            <span className="sftp-time">{when(entry.mtime)}</span>
          </div>
        );
      })}
    </div>
  );
}

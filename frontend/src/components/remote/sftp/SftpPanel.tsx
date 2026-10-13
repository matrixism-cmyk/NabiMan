import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useT } from '../../../i18n';
import RailContextMenu, { RailMenuItem } from '../RailContextMenu';
import SftpList, { Editing } from './SftpList';
import SftpTransfers from './SftpTransfers';
import { useTransfers } from './useTransfers';
import { useDragOut } from './useDragOut';
import { draggingDesktopFiles, draggingOwnRows, ownRows, SFTP_MIME, takeDrop } from './sftpDrop';
import {
  baseName, isFolder, joinPath, listDir, makeDir, movePath, removePath, SftpEntry, SftpListing, SftpSource, shellQuote,
} from './sftpApi';

interface Props {
  source: SftpSource;
  serverName: string;
  /** Types text into the terminal next to the panel. */
  onInsert: (text: string) => void;
  onClose: () => void;
}

const read = (key: string) => { try { return localStorage.getItem(key) || ''; } catch { return ''; } };
const write = (key: string, value: string) => { try { localStorage.setItem(key, value); } catch { /* private mode */ } };
const parentOf = (path: string) => {
  const trimmed = path.replace(/\/+$/, '');
  const at = trimmed.lastIndexOf('/');
  return at <= 0 ? '/' : trimmed.slice(0, at);
};

interface Menu { x: number; y: number; entry?: SftpEntry; opener: HTMLElement | null }

/**
 * The files of a saved server, beside its terminal. Desktop files dropped on
 * it are uploaded (folders too), rows dragged onto a folder are moved there,
 * and rows dragged onto the terminal type their paths.
 */
export default function SftpPanel({ source, serverName, onInsert, onClose }: Props) {
  const { t } = useT();
  const pathKey = `nabiman_sftp_path_${source.key}`;
  const writable = source.writable;
  const [listing, setListing] = useState<SftpListing | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const [showHidden, setShowHidden] = useState(() => read('nabiman_sftp_hidden') === '1');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dropOn, setDropOn] = useState<string | null>(null);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [menu, setMenu] = useState<Menu | null>(null);
  const anchor = useRef<string | null>(null);
  const current = useRef('');
  const fileInput = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async (path: string, fallbackHome = false) => {
    setLoading(true);
    setError('');
    try {
      const res = await listDir(source, path);
      if (res.success && res.data) {
        if (res.data.path !== current.current) setSelected(new Set());
        current.current = res.data.path;
        setListing(res.data);
        write(pathKey, res.data.path);
      } else if (fallbackHome && path) {
        // The folder remembered from last time is gone: start at home instead.
        await load('', false);
        return;
      } else {
        setError(res.message || t('common.error'));
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : t('common.error'));
    } finally {
      setLoading(false);
    }
  }, [source, pathKey, t]);

  useEffect(() => { load(read(pathKey), true); }, [load, pathKey]);

  const refresh = useCallback(() => load(current.current), [load]);
  const transfers = useTransfers(source, useCallback((dir: string) => {
    if (dir === current.current) refresh();
  }, [refresh]));

  const dragOut = useDragOut(source);
  const here = listing?.path || '';
  const pathOf = useCallback((name: string) => joinPath(current.current, name), []);
  const visible = useMemo(
    () => (listing?.entries || []).filter((e) => showHidden || !e.name.startsWith('.')),
    [listing, showHidden],
  );
  const chosen = visible.filter((e) => selected.has(e.name));

  const report = (failures: string[], success: string) => {
    if (failures.length) setError(failures.slice(0, 3).join(' · '));
    else setNotice(success);
    window.setTimeout(() => setNotice(''), 4000);
  };

  const open = (entry: SftpEntry) => {
    if (isFolder(entry)) load(pathOf(entry.name));
    else transfers.download([pathOf(entry.name)]);
  };

  const moveInto = async (paths: string[], dir: string) => {
    const failures: string[] = [];
    let moved = 0;
    for (const path of paths) {
      if (parentOf(path) === dir.replace(/\/+$/, '') || path === dir) continue;
      const res = await movePath(source, path, joinPath(dir, baseName(path)));
      if (res.success) moved += 1;
      else failures.push(`${baseName(path)}: ${res.message}`);
    }
    if (moved || failures.length) report(failures, t('sftp.moved', { count: moved, dir }));
    refresh();
  };

  const remove = async (entries: SftpEntry[]) => {
    if (!entries.length) return;
    const question = entries.length > 1 ? t('sftp.deleteMany', { count: entries.length })
      : isFolder(entries[0]) && entries[0].kind === 'dir' ? t('sftp.deleteFolder', { name: entries[0].name })
      : t('sftp.deleteFile', { name: entries[0].name });
    if (!window.confirm(question)) return;
    const failures: string[] = [];
    for (const entry of entries) {
      const res = await removePath(source, pathOf(entry.name));
      if (!res.success) failures.push(`${entry.name}: ${res.message}`);
    }
    report(failures, t('sftp.deleted', { count: entries.length - failures.length }));
    refresh();
  };

  const finishEdit = async (name: string | null) => {
    const edit = editing;
    setEditing(null);
    listRef.current?.focus();
    if (!edit || !name || name === edit.from) return;
    if (name.includes('/')) { setError(t('sftp.badName')); return; }
    const res = edit.mode === 'new'
      ? await makeDir(source, pathOf(name))
      : await movePath(source, pathOf(edit.from), pathOf(name));
    if (!res.success) setError(res.message || t('common.error'));
    else setSelected(new Set([name]));
    refresh();
  };

  const quoted = (entries: SftpEntry[]) => entries.map((e) => shellQuote(pathOf(e.name))).join(' ');

  const click = (e: React.MouseEvent, entry: SftpEntry) => {
    const name = entry.name;
    if (e.shiftKey && anchor.current) {
      const names = visible.map((v) => v.name);
      const [a, b] = [names.indexOf(anchor.current), names.indexOf(name)].sort((x, y) => x - y);
      if (a >= 0) { setSelected(new Set(names.slice(a, b + 1))); return; }
    }
    anchor.current = name;
    if (e.ctrlKey || e.metaKey) {
      setSelected((prev) => { const next = new Set(prev); if (!next.delete(name)) next.add(name); return next; });
    } else setSelected(new Set([name]));
  };

  const keyDown = (e: React.KeyboardEvent) => {
    if (editing) return;
    const names = visible.map((v) => v.name);
    const at = anchor.current ? names.indexOf(anchor.current) : -1;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const next = names[Math.max(0, Math.min(names.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)))];
      if (next) { anchor.current = next; setSelected(new Set([next])); }
    } else if (e.key === 'Enter' && chosen.length === 1) open(chosen[0]);
    else if (e.key === 'Backspace' && listing?.parent) load(listing.parent);
    else if (e.key === 'Delete' && writable) remove(chosen);
    else if (e.key === 'F2' && writable && chosen.length === 1) setEditing({ mode: 'rename', from: chosen[0].name });
    else if (e.key.toLowerCase() === 'a' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      setSelected(new Set(names));
    }
  };

  const dragStart = (e: React.DragEvent, entry: SftpEntry) => {
    const names = selected.has(entry.name) ? chosen.map((c) => c.name) : [entry.name];
    if (!selected.has(entry.name)) setSelected(new Set([entry.name]));
    const paths = names.map(pathOf);
    e.dataTransfer.setData(SFTP_MIME, JSON.stringify(paths));
    e.dataTransfer.setData('text/plain', paths.map(shellQuote).join(' '));
    e.dataTransfer.effectAllowed = 'copyMove';
    // A single file can also be dropped onto the desktop to save it.
    if (names.length === 1 && !isFolder(entry)) dragOut.attach(e.dataTransfer, paths[0], entry.name);
  };

  const dragOver = (e: React.DragEvent, dir: string) => {
    // A browse-only link takes nothing in; rows still drag onto the terminal.
    if (!writable) return;
    const own = draggingOwnRows(e.dataTransfer);
    if (!own && !draggingDesktopFiles(e.dataTransfer)) return;
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = own ? 'move' : 'copy';
    if (dropOn !== dir) setDropOn(dir);
  };

  const drop = (e: React.DragEvent, dir: string) => {
    if (!writable) return;
    e.preventDefault();
    e.stopPropagation();
    setDropOn(null);
    if (draggingOwnRows(e.dataTransfer)) {
      moveInto(ownRows(e.dataTransfer), dir);
    } else if (e.dataTransfer.types.includes('Files')) {
      const taken = takeDrop(e.dataTransfer);
      taken().then((dropped) => { if (dropped.files.length || dropped.dirs.length) transfers.upload(dir, dropped); });
    }
  };

  const menuItems = (entry?: SftpEntry): RailMenuItem[] => {
    if (!entry) {
      return [
        ...(writable ? [
          { key: 'mkdir', label: t('sftp.newFolder'), icon: '＋', onSelect: () => setEditing({ mode: 'new', from: '' }) },
          { key: 'upload', label: t('sftp.upload'), icon: '↑', onSelect: () => fileInput.current?.click() },
        ] : []),
        { key: 'refresh', label: t('sftp.refresh'), icon: '⟳', onSelect: refresh },
        { key: 'hidden', label: showHidden ? t('sftp.hideHidden') : t('sftp.showHidden'), icon: '◌', separated: true,
          onSelect: () => toggleHidden() },
      ];
    }
    const targets = selected.has(entry.name) ? chosen : [entry];
    const files = targets.filter((e) => !isFolder(e));
    const single = targets.length === 1;
    return [
      single && isFolder(entry)
        ? { key: 'open', label: t('sftp.open'), icon: '📂', onSelect: () => open(entry) }
        : { key: 'download', label: t('sftp.download'), icon: '↓', disabled: !files.length,
            onSelect: () => transfers.download(files.map((f) => pathOf(f.name))) },
      { key: 'insert', label: t('sftp.insertPath'), icon: '⌨', onSelect: () => onInsert(`${quoted(targets)} `) },
      { key: 'copy', label: t('sftp.copyPath'), icon: '⧉',
        onSelect: () => { navigator.clipboard?.writeText(quoted(targets)).catch(() => undefined); } },
      ...(writable ? [
        { key: 'rename', label: t('sftp.rename'), icon: '✎', disabled: !single,
          onSelect: () => setEditing({ mode: 'rename', from: entry.name }) },
        { key: 'delete', label: t('common.delete'), icon: '🗑', danger: true, separated: true,
          onSelect: () => remove(targets) },
      ] : []),
    ];
  };

  const toggleHidden = () => setShowHidden((v) => { write('nabiman_sftp_hidden', v ? '0' : '1'); return !v; });

  const crumbs = here.split('/').filter(Boolean).reduce<{ name: string; path: string }[]>(
    (all, part) => [...all, { name: part, path: `${all.length ? all[all.length - 1].path : ''}/${part}` }],
    [{ name: '/', path: '/' }],
  );

  return (
    <aside className="sftp" aria-label={t('sftp.panelLabel', { name: serverName })}
      onDragLeave={(e) => { if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setDropOn(null); }}>
      <div className="sftp-head">
        <strong>{t('sftp.title')}</strong>
        <div className="sftp-tools">
          <button type="button" onClick={() => listing?.parent && load(listing.parent)} disabled={!listing?.parent}
            title={t('sftp.up')} aria-label={t('sftp.up')}>↰</button>
          <button type="button" onClick={() => load('')} title={t('sftp.home')} aria-label={t('sftp.home')}>⌂</button>
          <button type="button" onClick={refresh} title={t('sftp.refresh')} aria-label={t('sftp.refresh')}>⟳</button>
          {writable && (
            <>
              <button type="button" onClick={() => setEditing({ mode: 'new', from: '' })}
                title={t('sftp.newFolder')} aria-label={t('sftp.newFolder')}>＋</button>
              <button type="button" onClick={() => fileInput.current?.click()}
                title={t('sftp.upload')} aria-label={t('sftp.upload')}>↑</button>
            </>
          )}
          <button type="button" className={showHidden ? 'is-on' : ''} onClick={toggleHidden}
            aria-pressed={showHidden} title={t('sftp.showHidden')} aria-label={t('sftp.showHidden')}>◌</button>
          <button type="button" className="sftp-hide" onClick={onClose}
            title={t('sftp.close')} aria-label={t('sftp.close')}>»</button>
        </div>
      </div>
      <nav className="sftp-crumbs" aria-label={t('sftp.location')}>
        {crumbs.map((c, i) => (
          <button type="button" key={c.path}
            className={`${i === crumbs.length - 1 ? 'is-here' : ''} ${dropOn === c.path ? 'is-drop' : ''}`}
            onClick={() => load(c.path)} onDragOver={(e) => dragOver(e, c.path)} onDrop={(e) => drop(e, c.path)}>
            {c.name}
          </button>
        ))}
      </nav>
      {(error || notice) && (
        <div className={`sftp-msg ${error ? 'is-error' : ''}`} role={error ? 'alert' : 'status'}>
          <span>{error || notice}</span>
          {error && <button type="button" className="sftp-link" onClick={() => { setError(''); refresh(); }}>{t('sftp.retry')}</button>}
        </div>
      )}
      <div
        ref={listRef}
        className={`sftp-body ${dropOn === here ? 'is-drop' : ''} ${loading ? 'is-loading' : ''}`}
        tabIndex={0}
        onKeyDown={keyDown}
        onClick={(e) => { if (e.target === e.currentTarget) setSelected(new Set()); }}
        onContextMenu={(e) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, opener: listRef.current }); }}
        onDragOver={(e) => dragOver(e, here)}
        onDrop={(e) => drop(e, here)}
      >
        {!listing && loading && <p className="sftp-empty">{t('sftp.connecting')}</p>}
        {listing && (
          <SftpList entries={visible} parent={listing.parent} selected={selected} dropOn={dropOn}
            editing={editing} pathOf={pathOf} onClick={click} onOpen={open}
            onUp={() => listing.parent && load(listing.parent)}
            onMenu={(e, entry) => {
              e.preventDefault();
              e.stopPropagation();
              if (!selected.has(entry.name)) { anchor.current = entry.name; setSelected(new Set([entry.name])); }
              setMenu({ x: e.clientX, y: e.clientY, entry, opener: e.currentTarget as HTMLElement });
            }}
            onDragStart={dragStart} onDragOverDir={dragOver} onDropDir={drop} onEditDone={finishEdit}
            onDragEnd={(entry) => dragOut.spent(pathOf(entry.name))}
            onPrefetch={(entry, now) => { if (!isFolder(entry)) dragOut.prefetch(pathOf(entry.name), now); }} />
        )}
        {listing && !visible.length && !editing && (
          <p className="sftp-empty">{t(writable ? 'sftp.empty' : 'sftp.emptyReadOnly')}</p>
        )}
        {listing?.truncated && <p className="sftp-empty">{t('sftp.truncated', { count: listing.entries.length })}</p>}
      </div>
      <SftpTransfers rows={transfers.rows} onCancel={transfers.cancel} onClear={transfers.clearFinished} />
      <p className="sftp-hint">
        {chosen.length > 1 ? t('sftp.selected', { count: chosen.length }) : t(writable ? 'sftp.dragHint' : 'sftp.dragHintReadOnly')}
      </p>
      <input ref={fileInput} type="file" multiple hidden onChange={(e) => {
        const files = Array.from(e.target.files || []);
        if (files.length) transfers.upload(here, { files: files.map((file) => ({ rel: file.name, file })), dirs: [] });
        e.target.value = '';
      }} />
      {menu && (
        <RailContextMenu x={menu.x} y={menu.y} opener={menu.opener}
          title={menu.entry ? (chosen.length > 1 ? t('sftp.selected', { count: chosen.length }) : menu.entry.name) : here}
          items={menuItems(menu.entry)} onClose={() => setMenu(null)} />
      )}
    </aside>
  );
}

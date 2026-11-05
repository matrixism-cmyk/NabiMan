import { RailMenuItem } from '../RailContextMenu';
import { previewOf } from './preview/previewKinds';
import { Editing } from './SftpList';
import { isFolder, SftpEntry } from './sftpApi';

/** What the panel's right-click menu works with. */
export interface MenuContext {
  t: (key: string, vars?: Record<string, string | number>) => string;
  writable: boolean;
  /** The open folder. */
  here: string;
  /** The selected rows, in list order. */
  chosen: SftpEntry[];
  selected: Set<string>;
  showHidden: boolean;
  canAttrs: boolean;
  pathOf: (name: string) => string;
  quoted: (entries: SftpEntry[]) => string;
  onInsert?: (text: string) => void;
  edit: (e: Editing) => void;
  pickFiles: () => void;
  pickFolder: () => void;
  saveFolder: (paths: string[]) => void;
  download: (items: { path: string; folder?: boolean }[]) => void;
  refresh: () => void;
  toggleHidden: () => void;
  preview: (name: string) => void;
  open: (entry: SftpEntry) => void;
  attrs: (entries: SftpEntry[]) => void;
  properties: (paths: string[]) => void;
  remove: (entries: SftpEntry[]) => void;
}

/** The menu for a row (and the selection it belongs to), or for the open folder itself. */
export function sftpMenu(c: MenuContext, entry?: SftpEntry): RailMenuItem[] {
  const { t } = c;
  if (!entry) {
    return [
      ...(c.writable ? [
        { key: 'mkdir', label: t('sftp.newFolder'), icon: '＋', onSelect: () => c.edit({ mode: 'new', from: '' }) },
        { key: 'upload', label: t('sftp.upload'), icon: '↑', onSelect: c.pickFiles },
        { key: 'uploadFolder', label: t('sftp.uploadFolder'), icon: '⇪', onSelect: c.pickFolder },
      ] : []),
      // The open folder itself, as a folder (not the filesystem root: far too much).
      ...(c.here && c.here !== '/' ? [{ key: 'saveHere', label: t('sftp.saveFolderHere'), icon: '⇩', onSelect: () => c.saveFolder([c.here]) }] : []),
      { key: 'refresh', label: t('sftp.refresh'), icon: '⟳', onSelect: c.refresh },
      { key: 'hidden', label: c.showHidden ? t('sftp.hideHidden') : t('sftp.showHidden'), icon: '◌', separated: true, onSelect: c.toggleHidden },
      ...(c.here ? [{ key: 'props', label: t('sftp.folderProperties'), icon: 'ℹ', onSelect: () => c.properties([c.here]) }] : []),
    ];
  }
  const targets = c.selected.has(entry.name) ? c.chosen : [entry];
  const saving = targets.map((e) => ({ path: c.pathOf(e.name), folder: isFolder(e) }));
  const single = targets.length === 1;
  return [
    ...(single && !isFolder(entry) && previewOf(entry.name)
      ? [{ key: 'preview', label: t('sftp.preview'), icon: '👁', onSelect: () => c.preview(entry.name) }]
      : []),
    single && isFolder(entry)
      ? { key: 'open', label: t('sftp.open'), icon: '📂', onSelect: () => c.open(entry) }
      : { key: 'download', label: t('sftp.download'), icon: '↓', onSelect: () => c.download(saving) },
    ...(single && isFolder(entry) ? [
      { key: 'saveFolder', label: t('sftp.saveFolder'), icon: '⇩', onSelect: () => c.saveFolder([c.pathOf(entry.name)]) },
      { key: 'zip', label: t('sftp.downloadFolder'), icon: '↓', onSelect: () => c.download(saving) },
    ] : []),
    ...(c.onInsert ? [{ key: 'insert', label: t('sftp.insertPath'), icon: '⌨', onSelect: () => c.onInsert?.(`${c.quoted(targets)} `) }] : []),
    { key: 'copy', label: t('sftp.copyPath'), icon: '⧉',
      onSelect: () => { navigator.clipboard?.writeText(c.quoted(targets)).catch(() => undefined); } },
    { key: 'props', label: t('sftp.properties'), icon: 'ℹ', onSelect: () => c.properties(targets.map((e) => c.pathOf(e.name))) },
    ...(c.writable ? [
      { key: 'rename', label: t('sftp.rename'), icon: '✎', disabled: !single, separated: true,
        onSelect: () => c.edit({ mode: 'rename', from: entry.name }) },
      ...(c.canAttrs ? [{ key: 'attrs', label: t('sftp.attrs'), icon: '🔐', onSelect: () => c.attrs(targets) }] : []),
      { key: 'delete', label: t('common.delete'), icon: '🗑', danger: true, separated: true,
        onSelect: () => c.remove(targets) },
    ] : []),
  ];
}

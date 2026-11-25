import React, { useCallback, useContext, useEffect, useRef, useState } from 'react';
import type { MutableRefObject, RefObject } from 'react';
import type { ILinkProvider, Terminal } from 'xterm';
import { useT } from '../../../shared/i18n';
import RailContextMenu, { RailMenuItem } from '../../remote/RailContextMenu';
import { TerminalFilesContext } from '../../remote/terminalFiles';
import { fileLinkUrl } from '../../remote/fileLink';
import PreviewDialog from '../../remote/sftp/preview/PreviewDialog';
import { baseName, saveFile } from '../../remote/sftp/sftpApi';
import { copyText } from '../ClaudeAddress';
import { pathsAtPoint, pathsOnRow, ScreenPath } from './fileLines';
import { attachLongPress } from './filePress';
import { FoundFile, resolveFile } from './fileResolve';
import { promptFolder } from './shellPrompt';
import '../ai-guide.css';

interface Options {
  termRef: MutableRefObject<Terminal | null>;
  containerRef: RefObject<HTMLDivElement | null>;
  /** The pane's identity: a new one means a new terminal to attach to. */
  storageKey: string;
  /** The terminal session attached now, if it is an account's own. */
  sessionId: () => string;
  /** The folder the session was started in, when it was started with one. */
  startDir?: string;
}

/**
 * Underlines the file names and paths under the pointer, as web links are.
 * A click on one does nothing new: the menu is on a long press.
 */
function linkProvider(term: Terminal, hint: () => string): ILinkProvider {
  return {
    provideLinks(line, callback) {
      const paths = pathsOnRow(term, line - 1);
      callback(paths.length ? paths.map((p) => ({
        range: p.range,
        text: p.path,
        decorations: { underline: true, pointerCursor: false },
        activate: () => undefined,
        hover: () => { if (term.element) term.element.title = hint(); },
        leave: () => { term.element?.removeAttribute('title'); },
      })) : undefined);
    },
  };
}

interface Menu { x: number; y: number; file: FoundFile }

/**
 * File names and paths printed in a terminal, pressed and held: a menu to
 * open the file in the file panel's preview, or to copy its path or a link
 * to it. Works wherever the terminal has a file panel beside it; elsewhere
 * (no file access) nothing is attached. Returns what to draw: the menu and
 * the preview.
 *
 * What the terminal prints is not trusted: it is only ever used as a path in
 * this session's own file requests, and shown as text.
 */
export function useTerminalFileLinks({ termRef, containerRef, storageKey, sessionId, startDir }: Options): React.ReactNode {
  const { t } = useT();
  const files = useContext(TerminalFilesContext);
  const source = files?.source ?? null;
  const [menu, setMenu] = useState<Menu | null>(null);
  const [preview, setPreview] = useState<FoundFile | null>(null);
  const [note, setNote] = useState('');
  const hintRef = useRef('');
  hintRef.current = t('termFile.hint');
  const look = useRef({ sessionId, startDir });
  look.current = { sessionId, startDir };

  useEffect(() => {
    const term = termRef.current;
    const el = containerRef.current;
    if (!term || !el || !source) return undefined;
    const links = term.registerLinkProvider(linkProvider(term, () => hintRef.current));
    const detach = attachLongPress<ScreenPath[], FoundFile>({
      el,
      candidateAt: (x, y) => {
        const paths = pathsAtPoint(term, x, y);
        return paths.length ? paths : null;
      },
      // Read when pressed, not on hover: the prompt above the line, then the server.
      resolve: (paths) => resolveFile(paths, {
        source, sessionId: look.current.sessionId() || undefined, startDir: look.current.startDir,
        promptDir: promptFolder(term, paths[0].range.start.y - 1),
      }),
      open: (file, x, y) => { setNote(''); setMenu({ file, x: x + 2, y: y + 2 }); },
    });
    return () => {
      detach();
      try { links.dispose(); } catch { /* the terminal went first */ }
    };
  }, [termRef, containerRef, source, storageKey]);

  const closeMenu = useCallback(() => setMenu(null), []);

  // The text under the menu moving away closes it: the terminal resized, scrolled, or the wheel turned.
  useEffect(() => {
    const term = termRef.current;
    const el = containerRef.current;
    if (!menu || !term || !el) return undefined;
    const resized = term.onResize(closeMenu);
    const scrolled = term.onScroll(closeMenu);
    el.addEventListener('wheel', closeMenu, { capture: true, passive: true });
    return () => {
      resized.dispose();
      scrolled.dispose();
      el.removeEventListener('wheel', closeMenu, true);
    };
  }, [menu, termRef, containerRef, closeMenu]);

  // "Copied" stays a moment, then the menu goes.
  useEffect(() => {
    if (!note) return undefined;
    const timer = window.setTimeout(closeMenu, 900);
    return () => window.clearTimeout(timer);
  }, [note, closeMenu]);

  if (!source) return null;
  const copy = (text: string) => { copyText(text).then((ok) => setNote(t(ok ? 'termFile.copied' : 'termFile.copyFailed'))); };
  const items = (file: FoundFile): RailMenuItem[] => [
    ...(file.folder
      ? [{ key: 'panel', label: t('termFile.showInPanel'), icon: '📂', onSelect: () => files?.showFolder(file.path) }]
      : [{ key: 'open', label: t('termFile.open'), icon: '👁', onSelect: () => setPreview(file) }]),
    { key: 'path', label: t('termFile.copyPath'), icon: '⧉', keepOpen: true, onSelect: () => copy(file.path) },
    // A link is for an account that can reach this server; a share link's visitor has none.
    ...(!file.folder && source.scope && !source.shareKey
      ? [{ key: 'link', label: t('termFile.copyLink'), icon: '🔗', keepOpen: true,
        onSelect: () => copy(fileLinkUrl({ scope: source.scope || '', path: file.path })) }]
      : []),
  ];

  return (
    <>
      {menu && (
        <RailContextMenu x={menu.x} y={menu.y} title={baseName(menu.file.path)} items={items(menu.file)}
          onClose={closeMenu} opener={termRef.current?.textarea} className="ai-guide-menu term-file-menu"
          footer={note ? <span role="status">{note}</span> : undefined} />
      )}
      {preview && (
        <PreviewDialog source={source} dir={preview.dir} files={[preview.entry]} start={preview.entry.name}
          onDownload={() => { saveFile(source, preview.path, preview.entry.name); }}
          onClose={() => { setPreview(null); termRef.current?.focus(); }} />
      )}
    </>
  );
}

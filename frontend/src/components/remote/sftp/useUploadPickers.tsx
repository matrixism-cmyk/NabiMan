import React, { useRef } from 'react';
import type { Dropped } from './sftpDrop';

/**
 * What a file picker chose, in the shape an upload takes. A folder pick gives
 * each file its path inside the folder (`webkitRelativePath`), so the folder
 * and its subfolders are recreated on the server, parents first. (A browser
 * lists only files, so a folder pick carries no empty subfolders.)
 */
export function fromPicked(files: File[]): Dropped {
  const dirs = new Set<string>();
  const picked = files.map((file) => {
    const rel = file.webkitRelativePath || file.name;
    const parts = rel.split('/');
    for (let i = 1; i < parts.length; i++) dirs.add(parts.slice(0, i).join('/'));
    return { rel, file };
  });
  const ordered = Array.from(dirs).sort((a, b) => a.split('/').length - b.split('/').length);
  return { files: picked, dirs: ordered };
}

/** Hidden "upload files" and "upload a folder" pickers, opened from buttons. */
export function useUploadPickers(onPick: (dropped: Dropped) => void) {
  const filesRef = useRef<HTMLInputElement>(null);
  const folderRef = useRef<HTMLInputElement>(null);
  const pick = useRef(onPick);
  pick.current = onPick;
  const change = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(e.target.files || []);
    if (files.length) pick.current(fromPicked(files));
    e.target.value = '';
  };
  // `webkitdirectory` is not in React's typed attributes; every current browser supports it.
  const folderAttrs = { webkitdirectory: '', directory: '' } as Record<string, string>;
  return {
    pickFiles: () => filesRef.current?.click(),
    pickFolder: () => folderRef.current?.click(),
    inputs: (
      <>
        <input ref={filesRef} type="file" multiple hidden onChange={change} />
        <input ref={folderRef} type="file" multiple hidden onChange={change} {...folderAttrs} />
      </>
    ),
  };
}

/**
 * Clipboard files on their way into a terminal.
 *
 * A terminal carries characters, not pictures, so an image on the clipboard
 * cannot be pasted into a remote shell the way text can — a CLI that pastes
 * images looks at the clipboard of the machine it runs on, which is the server.
 * Instead the bytes are uploaded and the terminal receives the path the server
 * saved them at, which the program on the other end can open.
 */
const API_BASE = process.env.REACT_APP_API_URL || '';

/** Clipboard types the server keeps; anything else is left to text pasting. */
const SUPPORTED = /^(image\/(png|jpeg|gif|webp|bmp|tiff|svg\+xml|avif)|application\/pdf)$/i;

export function isUploadable(type: string): boolean {
  return SUPPORTED.test(type.split(';')[0].trim());
}

/** Uploads one clipboard file and returns the path it was saved at. */
export async function uploadPastedFile(file: File | Blob): Promise<string | null> {
  if (!isUploadable(file.type)) return null;
  try {
    const res = await fetch(`${API_BASE}/api/terminal/paste-file`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/octet-stream',
        'X-Paste-Type': file.type,
        Authorization: `Bearer ${sessionStorage.getItem('nabiman_token') || ''}`,
      },
      body: file,
    });
    const json = await res.json();
    return json?.success && typeof json.data === 'string' ? json.data : null;
  } catch {
    return null;
  }
}

/**
 * Uploads every clipboard file it can and returns the paths, space separated,
 * ready to be pasted as a command argument. Empty when there was nothing to
 * upload or the upload failed.
 */
export async function pathsForClipboardFiles(files: Array<File | Blob>): Promise<string> {
  const usable = files.filter((f) => isUploadable(f.type));
  if (!usable.length) return '';
  const paths = await Promise.all(usable.map(uploadPastedFile));
  return paths.filter(Boolean).join(' ');
}

/** The files behind a `navigator.clipboard.read()` result, if any. */
export async function blobsFromClipboardItems(items: ClipboardItem[]): Promise<Blob[]> {
  const blobs: Blob[] = [];
  for (const item of items) {
    for (const type of item.types) {
      if (!isUploadable(type)) continue;
      try {
        blobs.push(await item.getType(type));
      } catch { /* the browser withheld it: nothing to paste */ }
    }
  }
  return blobs;
}

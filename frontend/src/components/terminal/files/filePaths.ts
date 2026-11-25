import { previewOf } from '../../remote/sftp/preview/previewKinds';

/**
 * File names and paths in a line of terminal output, found by shape alone:
 * nothing here asks the server. Whether a match is a real file is decided
 * later, once, when it is pressed (see ./fileResolve).
 *
 * Recognised: `/absolute/paths`, `~/home/paths`, `./relative` and
 * `folder/file.ext` paths, and bare `name.ext` files with a known extension.
 * A space ends a path unless the path is quoted or sits in Claude Code's file
 * card, `[file] <name> (<size>)`. Web addresses, version numbers, IP
 * addresses and domain names are left alone.
 */
export type PathKind = 'absolute' | 'home' | 'relative' | 'name';

export interface PathMatch {
  /** Where the path sits in the text: from `start` up to, not including, `end`. */
  start: number;
  end: number;
  /** The path as written, without a `:line:col` suffix. */
  path: string;
  kind: PathKind;
}

const MAX_PATH = 1024;
/** Extensions the preview does not know but that are files all the same. */
const MORE_EXTENSIONS = new Set([
  'gz', 'tgz', 'tar', 'bz2', 'xz', 'zst', '7z', 'rar', 'iso', 'img', 'bin', 'exe', 'dll', 'deb', 'rpm', 'dmg', 'msi',
  'db', 'sqlite', 'sqlite3', 'bak', 'dat', 'key', 'csr', 'p12', 'pfx', 'woff', 'woff2', 'ttf', 'otf', 'psd', 'heic',
  'tif', 'tiff', 'parquet', 'pkl', 'pt', 'onnx', 'npy', 'wasm', 'class', 'pyc', 'o', 'a',
]);

/** Whether a bare `name.ext` with this extension is worth treating as a file. */
export function knownExtension(ext: string): boolean {
  const lower = ext.toLowerCase();
  if (!/^[a-z0-9]{1,10}$/.test(lower) || !/[a-z]/.test(lower)) return false;
  return MORE_EXTENSIONS.has(lower) || previewOf(`x.${lower}`)?.kind !== 'file';
}

const extensionOf = (name: string) => {
  const dot = name.lastIndexOf('.');
  return dot > 0 && dot < name.length - 1 ? name.slice(dot + 1) : '';
};
/** Any ending that reads as an extension: letters and digits, with a letter. */
const extensionLike = (name: string) => /^[A-Za-z0-9_+-]{1,10}$/.test(extensionOf(name)) && /[A-Za-z]/.test(extensionOf(name));
/** `example.com`, `docs.rs:8080`: a first path part that is a host, not a folder. */
const hostLike = (part: string) => /^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}(:\d+)?$/.test(part);

/**
 * What kind of path `text` is, or null when it is not one. `sure` is for text
 * taken from a file card or from quotes, where spaces belong to the name.
 */
export function classify(text: string, sure: 'card' | 'quoted' | null = null): PathKind | null {
  if (text.length < 2 || text.length > MAX_PATH) return null;
  // eslint-disable-next-line no-control-regex -- control characters never belong to a path shown on screen
  if (/[\u0000-\u001f\u007f$\\]/.test(text) || text.includes('://') || text.startsWith('//')) return null;
  if (!sure && /\s/.test(text)) return null;
  if (text.startsWith('/')) {
    const parts = text.split('/').filter(Boolean);
    if (parts.length >= 2) return 'absolute';
    return parts.length === 1 && extensionLike(parts[0]) ? 'absolute' : null;
  }
  if (text.startsWith('~/')) return text.length > 2 ? 'home' : null;
  if (text.startsWith('./') || text.startsWith('../')) return /[^./]/.test(text) ? 'relative' : null;
  if (text.includes('/')) {
    const parts = text.replace(/\/$/, '').split('/');
    if (parts.some((p) => !p) || text.includes(':')) return null;
    if (sure === 'card') return 'relative';
    return extensionLike(parts[parts.length - 1]) && !hostLike(parts[0]) ? 'relative' : null;
  }
  if (sure === 'card') return 'name';
  // A bare name: `report.pdf`, not `2.1.112`, `example.com`, `a@b.com` or `--flag.x`.
  if (text.startsWith('-')) return null;
  if (!sure && /[@:=#;!?*&^{}]/.test(text)) return null;
  // `.env`, `.gitignore`: known by their whole name.
  if (text.startsWith('.') && previewOf(text)?.kind === 'text') return 'name';
  return knownExtension(extensionOf(text)) ? 'name' : null;
}

const CARD = /\[(?:file|image|pdf|document)\]\s+(\S(?:.*?\S)?)\s+\(\s*[\d.,]+\s*(?:[KMGT]i?B|B|bytes?)\s*\)/gi;
const QUOTED = /(["'`])((?:(?!\1).){2,400})\1/g;
/** A run of text that could be one path: no spaces, quotes, angle brackets, bars or box lines. */
const TOKEN = /[^\s"'`<>|─-╿]+/g;
const LEADING = /^[([{@,;*]+/;
const TRAILING = /[.,;:!?*…。、]+$/;

/** `token` without what surrounds a path in a sentence; `skip` is how much went from its start. */
function trimToken(token: string): { text: string; skip: number } {
  let text = token;
  let skip = text.match(LEADING)?.[0].length || 0;
  text = text.slice(skip);
  // `--out=/tmp/x.log`: the path is what follows the last `=`.
  const assign = text.search(/=(?=[/~.])[^=]*$/);
  if (assign >= 0) { skip += assign + 1; text = text.slice(assign + 1); }
  for (;;) {
    const before = text;
    text = text.replace(TRAILING, '');
    // A closing bracket goes only when nothing inside the path opened it: `report(1).pdf` keeps its own.
    for (const [open, close] of [['(', ')'], ['[', ']'], ['{', '}']]) {
      if (text.endsWith(close) && text.split(close).length > text.split(open).length) text = text.slice(0, -1);
    }
    if (text === before) break;
  }
  // Korean and Japanese put particles right after a name: `report.pdf를`, `report.pdfです`.
  text = text.replace(/(\.[A-Za-z0-9]{1,10})[^\u0000-\u007f]+$/, '$1'); // eslint-disable-line no-control-regex -- "not ASCII" is the point
  // `file.ts:12` and `file.ts:12:5` point into the file.
  text = text.replace(/:\d+(?::\d+)?$/, '');
  return { text, skip };
}

/**
 * Every path in `text`, in the order they appear, a longer one first where
 * two start together. Matches may overlap — a quoted `"final report.pdf"`
 * holds `report.pdf` too — and whoever resolves them tries the longest first.
 */
export function findPaths(text: string): PathMatch[] {
  const found: PathMatch[] = [];
  const add = (start: number, path: string, kind: PathKind | null) => {
    const end = start + path.length;
    if (kind && !found.some((m) => m.start === start && m.end === end)) found.push({ start, end, path, kind });
  };
  for (const m of Array.from(text.matchAll(CARD))) {
    add((m.index || 0) + m[0].indexOf(m[1], m[0].indexOf(']')), m[1], classify(m[1], 'card'));
  }
  for (const m of Array.from(text.matchAll(QUOTED))) {
    // Quotes matter only around spaces; without one the plain scan below finds the path.
    if (/\s/.test(m[2]) && !/^\s|\s$/.test(m[2])) add((m.index || 0) + 1, m[2], classify(m[2], 'quoted'));
  }
  for (const m of Array.from(text.matchAll(TOKEN))) {
    if (m[0].includes('://')) continue;
    const { text: core, skip } = trimToken(m[0]);
    add((m.index || 0) + skip, core, classify(core));
  }
  return found.sort((a, b) => a.start - b.start || b.end - a.end);
}

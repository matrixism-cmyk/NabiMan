import { apiRequest } from '../../../shared/api/useApi';
import { isFolder, joinPath, listDir, SftpEntry, SftpListing, SftpSource } from '../../remote/sftp/sftpApi';
import type { PathMatch } from './filePaths';

/**
 * From a path printed in a terminal to a file on that terminal's server.
 *
 * What is printed is only ever a path handed to the file panel's own listing
 * request for this session's server, which checks the account like any other
 * file request. A press asks once per folder, and the answer is kept for a few
 * seconds so pressing again (or the menu's actions) does not ask again.
 */
export interface FoundFile {
  /** The absolute path. */
  path: string;
  /** The folder it is in. */
  dir: string;
  entry: SftpEntry;
  folder: boolean;
}

/** Where to look for a name printed without its folder. */
export interface Lookup {
  source: SftpSource;
  /** The terminal session, when it is an account's own: the server says where its Claude sessions and its shell are. */
  sessionId?: string;
  /** The folder a session was started in, when known. */
  startDir?: string;
  /** The folder the shell prompt above the pressed line shows (`/abs`, `~`, `~/sub`), if any: see ./shellPrompt. */
  promptDir?: string | null;
  /** Full paths printed elsewhere in the terminal that end in a name (see ./fileMentions): tried when its folders do not have it. */
  mentioned?: (name: string) => string[];
}

const KEEP_MS = 10000;
/** A shell's folder changes with every `cd`: its answer is kept only for the press that asked. */
const KEEP_CWD_MS = 1000;
/** Folders a bare name is looked for in at most; one press lists twice as many folders at most. */
const MAX_FOLDERS = 6;
const MAX_KEPT = 60;

interface Kept<T> { at: number; value: Promise<T> }
const listings = new Map<string, Kept<SftpListing | null>>();
const homes = new Map<string, Promise<string | null>>();
const folders = new Map<string, Kept<string[]>>();
const shells = new Map<string, Kept<string | null>>();

/** Forgets every kept answer (tests). */
export function forgetFiles(): void {
  listings.clear();
  homes.clear();
  folders.clear();
  shells.clear();
}

function kept<T>(map: Map<string, Kept<T>>, key: string, make: () => Promise<T>, keepMs = KEEP_MS): Promise<T> {
  const now = Date.now();
  const have = map.get(key);
  if (have && now - have.at < keepMs) return have.value;
  if (map.size >= MAX_KEPT) map.clear();
  const value = make();
  map.set(key, { at: now, value });
  return value;
}

function listing(source: SftpSource, dir: string): Promise<SftpListing | null> {
  return kept(listings, `${source.base}\n${dir}`, () =>
    listDir(source, dir).then((res) => (res.success && res.data ? res.data : null)).catch(() => null));
}

/** The account's home folder there: what `~` stands for. Asked once. */
function home(source: SftpSource): Promise<string | null> {
  let value = homes.get(source.base);
  if (!value) {
    value = listDir(source, '').then((res) => {
      if (!res.success || !res.data) return null;
      // The answer is the home folder's listing too: keep it as one.
      listings.set(`${source.base}\n${res.data.path}`, { at: Date.now(), value: Promise.resolve(res.data) });
      return res.data.path;
    }).catch(() => null);
    homes.set(source.base, value);
    value.then((path) => { if (!path) homes.delete(source.base); });
  }
  return value;
}

/** `/a/./b/../c` as `/a/c`; null for anything but an absolute path. */
export function normalize(path: string): string | null {
  if (!path.startsWith('/')) return null;
  const out: string[] = [];
  for (const part of path.split('/')) {
    if (!part || part === '.') continue;
    if (part === '..') out.pop();
    else out.push(part);
  }
  return `/${out.join('/')}`;
}

const readStored = (key: string) => { try { return localStorage.getItem(key) || ''; } catch { return ''; } };

/** Where the session's Claude sessions work, as the server knows it. */
function claudeFolders(sid: string): Promise<string[]> {
  return kept(folders, sid, () =>
    apiRequest<{ claude: { cwd: string }[] }>(`/api/terminal/sessions/${encodeURIComponent(sid)}/claude`)
      .then((res) => (res.success ? (res.data?.claude || []).map((c) => c.cwd) : []))
      .catch(() => []));
}

/** The folder the session's shell is in now; the server knows it for a shell on its own machine only. */
function shellFolder(sid: string): Promise<string | null> {
  return kept(shells, sid, () =>
    apiRequest<{ cwd: string | null }>(`/api/terminal/sessions/${encodeURIComponent(sid)}/cwd`)
      .then((res) => (res.success ? res.data?.cwd || null : null))
      .catch(() => null), KEEP_CWD_MS);
}

/**
 * The folders a bare name may be in, most likely first:
 * the folder the prompt above the pressed line shows (where that output was
 * printed, on whichever machine the shell is); where Claude Code works; where
 * the shell is now; where the session started; the folder the file panel
 * shows; home.
 */
async function workingFolders(look: Lookup): Promise<string[]> {
  const out: string[] = [];
  const add = (dir?: string | null) => { if (dir && dir.startsWith('/') && !out.includes(dir)) out.push(dir); };
  const own = look.sessionId && !look.source.shareKey ? look.sessionId : '';
  // Asked together. The shell's folder is this machine's: only for this machine's files.
  const [claude, shell] = await Promise.all([
    own ? claudeFolders(own) : [],
    own && look.source.scope === 'local' ? shellFolder(own) : null,
  ]);
  const shown = look.promptDir;
  if (shown && shown.startsWith('~')) {
    const base = await home(look.source);
    if (base) add(normalize(joinPath(base, shown.slice(2))));
  } else if (shown) {
    add(normalize(shown));
  }
  claude.slice(0, 2).forEach(add);
  add(shell);
  add(look.startDir);
  // The folder the file panel beside the terminal is showing, or last showed.
  add(readStored(`nabiman_sftp_path_${look.source.key}`));
  add(await home(look.source));
  return out.slice(0, MAX_FOLDERS);
}

/** The absolute paths `match` may mean, most likely first. */
async function candidates(match: PathMatch, look: Lookup): Promise<string[]> {
  if (match.kind === 'absolute') return [match.path];
  if (match.kind === 'home') {
    const base = await home(look.source);
    return base ? [joinPath(base, match.path.slice(2))] : [];
  }
  const dirs = await workingFolders(look);
  const out = dirs.map((dir) => joinPath(dir, match.path));
  // A diff names files as `a/src/x.ts` and `b/src/x.ts`.
  if (/^[ab]\//.test(match.path)) out.push(...dirs.map((dir) => joinPath(dir, match.path.slice(2))));
  return out;
}

async function stat(source: SftpSource, raw: string): Promise<FoundFile | null> {
  const path = normalize(raw);
  if (!path || path === '/') return null;
  const at = path.lastIndexOf('/');
  const dir = at === 0 ? '/' : path.slice(0, at);
  const name = path.slice(at + 1);
  const entry = (await listing(source, dir))?.entries.find((e) => e.name === name);
  return entry ? { path, dir, entry, folder: isFolder(entry) } : null;
}

/**
 * The first of `matches` (the paths under one press, longest first) that is
 * there, or null. At most a handful of folders are asked about.
 */
export async function resolveFile(matches: PathMatch[], look: Lookup): Promise<FoundFile | null> {
  let asked = 0;
  const seen = new Set<string>();
  for (const match of matches) {
    for (const path of await candidates(match, look)) {
      if (seen.has(path)) continue;
      seen.add(path);
      if (asked >= MAX_FOLDERS * 2) return null;
      asked += 1;
      const found = await stat(look.source, path);
      if (found) return found;
    }
  }
  // Not in any folder a bare name is looked for in: the same name printed with its folder, elsewhere on the terminal.
  for (const match of matches) {
    if (match.kind !== 'name' && match.kind !== 'relative') continue;
    for (const written of look.mentioned?.(match.path) || []) {
      const [path] = await candidates({ ...match, path: written, kind: written.startsWith('~') ? 'home' : 'absolute' }, look);
      if (!path || seen.has(path)) continue;
      seen.add(path);
      const found = await stat(look.source, path);
      if (found) return found;
    }
  }
  return null;
}

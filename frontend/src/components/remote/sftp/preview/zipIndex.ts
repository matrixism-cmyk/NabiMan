/**
 * Reading what a ZIP holds from its central directory alone: the end of the
 * file says where the directory is, and the directory lists every entry. A
 * large archive is read in two small ranges, never whole. ZIP64 included.
 */

export interface ZipEntry {
  name: string;
  folder: boolean;
  size: number;
  compressed: number;
  /** Seconds since the epoch, as the archive's DOS time (local, so read as UTC). */
  mtime: number | null;
  encrypted: boolean;
}

/** Where the directory is, found in the archive's last bytes. */
export interface ZipEnd {
  entries: number;
  dirOffset: number;
  dirSize: number;
  /** Where the end record itself starts, for checking a ZIP64 locator. */
  endOffset: number;
}

const EOCD = 0x06054b50;
const ZIP64_LOCATOR = 0x07064b50;
const ZIP64_EOCD = 0x06064b50;
const CENTRAL = 0x02014b50;

const u16 = (b: Uint8Array, at: number) => b[at] | (b[at + 1] << 8);
const u32 = (b: Uint8Array, at: number) => (b[at] | (b[at + 1] << 8) | (b[at + 2] << 16) | (b[at + 3] << 24)) >>> 0;
const u64 = (b: Uint8Array, at: number) => u32(b, at) + u32(b, at + 4) * 2 ** 32;

/** How many bytes from the end hold the end record: it plus a 64 KB comment at most. */
export const TAIL = 22 + 0xffff + 20;

/**
 * The end-of-directory record in `tail` (the file's last bytes, which begin
 * at `tailStart`), and the ZIP64 record's offset if the archive needs it.
 */
export function findEnd(tail: Uint8Array, tailStart: number): { end: ZipEnd; zip64At: number | null } | null {
  for (let i = tail.length - 22; i >= 0; i -= 1) {
    if (u32(tail, i) !== EOCD) continue;
    const end = { entries: u16(tail, i + 10), dirSize: u32(tail, i + 12), dirOffset: u32(tail, i + 16), endOffset: tailStart + i };
    const loc = i - 20;
    if (loc >= 0 && u32(tail, loc) === ZIP64_LOCATOR) return { end, zip64At: u64(tail, loc + 8) };
    return { end, zip64At: null };
  }
  return null;
}

/** The ZIP64 end record: the real counts when the classic ones overflow. */
export function readZip64End(record: Uint8Array, endOffset: number): ZipEnd | null {
  if (record.length < 56 || u32(record, 0) !== ZIP64_EOCD) return null;
  return { entries: u64(record, 32), dirSize: u64(record, 40), dirOffset: u64(record, 48), endOffset };
}

function dosTime(time: number, date: number): number | null {
  if (!date) return null;
  const ms = Date.UTC(1980 + (date >> 9), ((date >> 5) & 15) - 1, date & 31, time >> 11, (time >> 5) & 63, (time & 31) * 2);
  return Number.isFinite(ms) ? ms / 1000 : null;
}

/** Names are UTF-8 when the archive says so; otherwise often the maker's code page (Korean Windows: CP949). */
function decodeName(bytes: Uint8Array, utf8: boolean): string {
  if (utf8) return new TextDecoder('utf-8').decode(bytes);
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder('euc-kr').decode(bytes);
  }
}

/** Every entry of a central directory, up to `limit`. */
export function readDirectory(dir: Uint8Array, limit = 20000): ZipEntry[] {
  const out: ZipEntry[] = [];
  let at = 0;
  while (at + 46 <= dir.length && out.length < limit && u32(dir, at) === CENTRAL) {
    const flags = u16(dir, at + 8);
    let compressed = u32(dir, at + 20);
    let size = u32(dir, at + 24);
    const nameLen = u16(dir, at + 28);
    const extraLen = u16(dir, at + 30);
    const commentLen = u16(dir, at + 32);
    const nameBytes = dir.subarray(at + 46, at + 46 + nameLen);
    // ZIP64 sizes live in the extra field (header id 1) when these overflow.
    let x = at + 46 + nameLen;
    const extraEnd = x + extraLen;
    while (x + 4 <= extraEnd) {
      const id = u16(dir, x);
      const len = u16(dir, x + 2);
      if (id === 1) {
        let f = x + 4;
        if (size === 0xffffffff && f + 8 <= x + 4 + len) { size = u64(dir, f); f += 8; }
        if (compressed === 0xffffffff && f + 8 <= x + 4 + len) { compressed = u64(dir, f); }
      }
      x += 4 + len;
    }
    const name = decodeName(nameBytes, (flags & 0x800) !== 0);
    out.push({
      name,
      folder: name.endsWith('/'),
      size,
      compressed,
      mtime: dosTime(u16(dir, at + 12), u16(dir, at + 14)),
      encrypted: (flags & 1) !== 0,
    });
    at = extraEnd + commentLen;
  }
  return out;
}

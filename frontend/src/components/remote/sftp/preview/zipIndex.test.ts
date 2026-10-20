/** @jest-environment node */
import { findEnd, readDirectory, readZip64End } from './zipIndex';

/** A tiny archive's central directory and end record, built by hand. */
function archive(names: { name: string; utf8?: boolean; bytes?: Uint8Array }[]) {
  const parts: number[] = [];
  const le = (v: number, n: number) => { for (let i = 0; i < n; i += 1) parts.push((v >>> (8 * i)) & 0xff); };
  for (const n of names) {
    const raw = n.bytes || new TextEncoder().encode(n.name);
    le(0x02014b50, 4); le(20, 2); le(20, 2); le(n.utf8 ? 0x800 : 0, 2); le(8, 2);
    le((10 << 11) | (30 << 5), 2); le(((2026 - 1980) << 9) | (10 << 5) | 20, 2);
    le(0, 4); le(50, 4); le(120, 4); le(raw.length, 2); le(0, 2); le(0, 2); le(0, 2); le(0, 2); le(0, 4); le(0, 4);
    parts.push(...Array.from(raw));
  }
  const dirSize = parts.length;
  le(0x06054b50, 4); le(0, 2); le(0, 2); le(names.length, 2); le(names.length, 2); le(dirSize, 4); le(1000, 4); le(0, 2);
  return { bytes: new Uint8Array(parts), dirSize };
}

test('the end record says where the directory is', () => {
  const { bytes, dirSize } = archive([{ name: 'a.txt' }, { name: 'docs/', utf8: true }]);
  const found = findEnd(bytes, 5000);
  expect(found?.zip64At).toBeNull();
  expect(found?.end).toEqual({ entries: 2, dirSize, dirOffset: 1000, endOffset: 5000 + dirSize });
  const entries = readDirectory(bytes.subarray(0, dirSize));
  expect(entries.map((e) => [e.name, e.folder, e.size, e.compressed])).toEqual([['a.txt', false, 120, 50], ['docs/', true, 120, 50]]);
  expect(new Date(entries[0].mtime! * 1000).toISOString()).toBe('2026-10-20T10:30:00.000Z');
});

test('Korean names from Windows (CP949) and UTF-8 both read', () => {
  const cp949 = new Uint8Array([0xc7, 0xd1, 0xb1, 0xdb, 0x2e, 0x74, 0x78, 0x74]); // 한글.txt
  const { bytes, dirSize } = archive([{ name: '', bytes: cp949 }, { name: '보고서.hwp', utf8: true }]);
  expect(readDirectory(bytes.subarray(0, dirSize)).map((e) => e.name)).toEqual(['한글.txt', '보고서.hwp']);
});

test('no end record, no listing; a ZIP64 record gives the real counts', () => {
  expect(findEnd(new Uint8Array(100), 0)).toBeNull();
  const rec = new Uint8Array(56);
  const view = new DataView(rec.buffer);
  view.setUint32(0, 0x06064b50, true);
  view.setUint32(32, 70000, true);
  view.setUint32(40, 9000000, true);
  view.setUint32(48, 5, true); view.setUint32(52, 1, true); // 4294967301
  expect(readZip64End(rec, 123)).toEqual({ entries: 70000, dirSize: 9000000, dirOffset: 4294967301, endOffset: 123 });
});

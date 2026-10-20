/** Telling what a file is from its first bytes, and showing bytes as hex. */

const MAGIC: [number[], string][] = [
  [[0x7f, 0x45, 0x4c, 0x46], 'ELF 실행 파일 / 라이브러리'],
  [[0x4d, 0x5a], 'Windows 실행 파일 (EXE/DLL)'],
  [[0xcf, 0xfa, 0xed, 0xfe], 'macOS 실행 파일 (Mach-O)'],
  [[0x1f, 0x8b], 'gzip 압축'],
  [[0x42, 0x5a, 0x68], 'bzip2 압축'],
  [[0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00], 'xz 압축'],
  [[0x28, 0xb5, 0x2f, 0xfd], 'zstd 압축'],
  [[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c], '7-Zip 압축'],
  [[0x52, 0x61, 0x72, 0x21, 0x1a, 0x07], 'RAR 압축'],
  [[0x50, 0x4b, 0x03, 0x04], 'ZIP 압축'],
  [[0x25, 0x50, 0x44, 0x46], 'PDF 문서'],
  [[0x89, 0x50, 0x4e, 0x47], 'PNG 그림'],
  [[0xff, 0xd8, 0xff], 'JPEG 그림'],
  [[0x47, 0x49, 0x46, 0x38], 'GIF 그림'],
  [[0x53, 0x51, 0x4c, 0x69, 0x74, 0x65, 0x20, 0x66], 'SQLite 데이터베이스'],
  [[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1], '옛 Office·한글 문서 (OLE: doc/xls/ppt/hwp)'],
  [[0x00, 0x61, 0x73, 0x6d], 'WebAssembly'],
  [[0xca, 0xfe, 0xba, 0xbe], 'Java 클래스 / macOS 유니버설 바이너리'],
  [[0x4f, 0x67, 0x67, 0x53], 'Ogg 미디어'],
  [[0x1a, 0x45, 0xdf, 0xa3], 'Matroska/WebM 미디어'],
  [[0x49, 0x44, 0x33], 'MP3 음성'],
  [[0x66, 0x4c, 0x61, 0x43], 'FLAC 음성'],
  [[0x2d, 0x2d, 0x2d, 0x2d, 0x2d, 0x42, 0x45, 0x47, 0x49, 0x4e], 'PEM (인증서·키)'],
];

/** A description of the file from its first bytes, or `null` when unknown. */
export function sniff(head: Uint8Array): string | null {
  for (const [sig, name] of MAGIC) {
    if (sig.every((b, i) => head[i] === b)) return name;
  }
  // "ustar" at 257: a tar archive.
  if (head.length > 262 && String.fromCharCode(...Array.from(head.subarray(257, 262))) === 'ustar') return 'tar 묶음';
  if (head.length >= 12 && String.fromCharCode(...Array.from(head.subarray(4, 8))) === 'ftyp') return 'MP4/MOV/HEIC 미디어';
  if (head.length >= 12 && String.fromCharCode(...Array.from(head.subarray(0, 4))) === 'RIFF') return 'RIFF 미디어 (WAV/AVI/WebP)';
  return null;
}

/** `offset  xx xx … xx  |ascii|`, 16 bytes a line, as `hexdump -C` prints. */
export function hexLines(bytes: Uint8Array, base = 0): string[] {
  const lines: string[] = [];
  for (let at = 0; at < bytes.length; at += 16) {
    const row = Array.from(bytes.subarray(at, at + 16));
    const hex = row.map((b) => b.toString(16).padStart(2, '0'));
    const left = hex.slice(0, 8).join(' ').padEnd(23);
    const right = hex.slice(8).join(' ').padEnd(23);
    const ascii = row.map((b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : '.')).join('');
    lines.push(`${(base + at).toString(16).padStart(8, '0')}  ${left}  ${right}  |${ascii}|`);
  }
  return lines;
}

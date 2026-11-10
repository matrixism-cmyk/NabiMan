/** Telling what a file is from its first bytes, and showing bytes as hex. */

/** First bytes and the translation key (`sniff.*`) of what they make the file. */
const MAGIC: [number[], string][] = [
  [[0x7f, 0x45, 0x4c, 0x46], 'sniff.elf'],
  [[0x4d, 0x5a], 'sniff.exe'],
  [[0xcf, 0xfa, 0xed, 0xfe], 'sniff.macho'],
  [[0x1f, 0x8b], 'sniff.gzip'],
  [[0x42, 0x5a, 0x68], 'sniff.bzip2'],
  [[0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00], 'sniff.xz'],
  [[0x28, 0xb5, 0x2f, 0xfd], 'sniff.zstd'],
  [[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c], 'sniff.7z'],
  [[0x52, 0x61, 0x72, 0x21, 0x1a, 0x07], 'sniff.rar'],
  [[0x50, 0x4b, 0x03, 0x04], 'sniff.zip'],
  [[0x25, 0x50, 0x44, 0x46], 'sniff.pdf'],
  [[0x89, 0x50, 0x4e, 0x47], 'sniff.png'],
  [[0xff, 0xd8, 0xff], 'sniff.jpeg'],
  [[0x47, 0x49, 0x46, 0x38], 'sniff.gif'],
  [[0x53, 0x51, 0x4c, 0x69, 0x74, 0x65, 0x20, 0x66], 'sniff.sqlite'],
  [[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1], 'sniff.ole'],
  [[0x00, 0x61, 0x73, 0x6d], 'sniff.wasm'],
  [[0xca, 0xfe, 0xba, 0xbe], 'sniff.javaClass'],
  [[0x4f, 0x67, 0x67, 0x53], 'sniff.ogg'],
  [[0x1a, 0x45, 0xdf, 0xa3], 'sniff.matroska'],
  [[0x49, 0x44, 0x33], 'sniff.mp3'],
  [[0x66, 0x4c, 0x61, 0x43], 'sniff.flac'],
  [[0x2d, 0x2d, 0x2d, 0x2d, 0x2d, 0x42, 0x45, 0x47, 0x49, 0x4e], 'sniff.pem'],
];

/** The translation key of what the file is from its first bytes, or `null` when unknown. */
export function sniff(head: Uint8Array): string | null {
  for (const [sig, name] of MAGIC) {
    if (sig.every((b, i) => head[i] === b)) return name;
  }
  // "ustar" at 257: a tar archive.
  if (head.length > 262 && String.fromCharCode(...Array.from(head.subarray(257, 262))) === 'ustar') return 'sniff.tar';
  if (head.length >= 12 && String.fromCharCode(...Array.from(head.subarray(4, 8))) === 'ftyp') return 'sniff.mp4';
  if (head.length >= 12 && String.fromCharCode(...Array.from(head.subarray(0, 4))) === 'RIFF') return 'sniff.riff';
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

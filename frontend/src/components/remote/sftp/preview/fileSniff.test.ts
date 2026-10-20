import { hexLines, sniff } from './fileSniff';

test('common binaries are told apart by their first bytes', () => {
  expect(sniff(new Uint8Array([0x7f, 0x45, 0x4c, 0x46, 2, 1]))).toMatch(/ELF/);
  expect(sniff(new Uint8Array([0x1f, 0x8b, 8, 0]))).toMatch(/gzip/);
  const tar = new Uint8Array(512);
  tar.set([0x75, 0x73, 0x74, 0x61, 0x72], 257);
  expect(sniff(tar)).toMatch(/tar/);
  expect(sniff(new Uint8Array([1, 2, 3, 4]))).toBeNull();
});

test('hex looks like hexdump -C', () => {
  const lines = hexLines(new Uint8Array([0x48, 0x69, 0x00, 0xff, ...Array(14).fill(0x41)]), 0x10);
  expect(lines[0]).toBe('00000010  48 69 00 ff 41 41 41 41  41 41 41 41 41 41 41 41  |Hi..AAAAAAAAAAAA|');
  expect(lines[1]).toBe('00000020  41 41' + ' '.repeat(18) + '  ' + ' '.repeat(23) + '  |AA|');
});

/** @jest-environment node */
import { decode, linesOf, looksBinary, splitHighlighted } from './textContent';
import { previewOf } from './previewKinds';

const bytes = (...values: number[]) => new Uint8Array(values);

test('a NUL near the start means binary', () => {
  expect(looksBinary(new TextEncoder().encode('plain text\n'))).toBe(false);
  expect(looksBinary(bytes(0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x00))).toBe(true);
});

test('UTF-8 is read as is; other bytes fall back to EUC-KR', () => {
  expect(decode(new TextEncoder().encode('한글\r\nline'), 'auto')).toEqual({ text: '한글\nline', encoding: 'utf-8' });
  // "한글" in EUC-KR.
  expect(decode(bytes(0xc7, 0xd1, 0xb1, 0xdb), 'auto')).toEqual({ text: '한글', encoding: 'euc-kr' });
  expect(decode(bytes(0xe9), 'iso-8859-1').text).toBe('é');
});

test('a final newline does not count as a line', () => {
  expect(linesOf('a\nb\n')).toEqual(['a', 'b']);
  expect(linesOf('')).toEqual(['']);
  expect(linesOf('\n\n')).toEqual(['', '']);
});

test('colour spans are closed and reopened across lines', () => {
  const html = 'x = <span class="hljs-comment">/* one\ntwo */</span> y\nz';
  expect(splitHighlighted(html)).toEqual([
    'x = <span class="hljs-comment">/* one</span>',
    '<span class="hljs-comment">two */</span> y',
    'z',
  ]);
});

test('texts are known by extension and by name', () => {
  expect(previewOf('main.rs')).toEqual(expect.objectContaining({ kind: 'text', language: 'rust' }));
  expect(previewOf('Dockerfile')?.language).toBe('dockerfile');
  expect(previewOf('Dockerfile.dev')?.language).toBe('dockerfile');
  expect(previewOf('.bashrc')?.language).toBe('bash');
  expect(previewOf('.env.production')?.language).toBe('bash');
  expect(previewOf('nginx.conf')?.language).toBe('nginx');
  expect(previewOf('site.conf')?.language).toBe('ini');
  expect(previewOf('.htaccess')?.language).toBe('apache');
  expect(previewOf('README')?.language).toBe('plaintext');
  expect(previewOf('app.log')?.kind).toBe('text');
  expect(previewOf('archive.tar.gz')).toBeNull();
  expect(previewOf('photo.png')?.kind).toBe('image');
});

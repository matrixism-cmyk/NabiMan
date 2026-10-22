import { HangulComposer, needsHangulComposer } from './hangul';

/** What a shell's line shows after the composer's output: Backspace takes one character back. */
function typed(keys: string[]): string {
  const composer = new HangulComposer();
  let line = '';
  for (const key of keys) {
    for (const ch of Array.from(composer.feed(key))) line = ch === '\x7f' ? Array.from(line).slice(0, -1).join('') : line + ch;
  }
  return line;
}
const letters = (s: string) => Array.from(s);

test('letters sent one by one become syllables', () => {
  expect(typed(letters('ㅇㅣㄹㅓㅎㄱㅔ'))).toBe('이렇게');
  expect(typed(letters('ㅇㅏㅇㅣㅍㅗㄴ ㅋㅡㄹㅗㅁ'))).toBe('아이폰 크롬');
  expect(typed(letters('ㅎㅏㄴㄱㅡㄹ ㅇㅣㅂㄹㅕㄱ'))).toBe('한글 입력');
});

test('compound vowels and finals, and a final that moves on', () => {
  expect(typed(letters('ㄱㅗㅏ'))).toBe('과');
  expect(typed(letters('ㅇㅡㅣ'))).toBe('의');
  expect(typed(letters('ㄷㅏㄹㄱ'))).toBe('닭');
  expect(typed(letters('ㄷㅏㄹㄱㅇㅣ'))).toBe('닭이');
  expect(typed(letters('ㄷㅏㄹㄱㅏ'))).toBe('달가');
  expect(typed(letters('ㄱㅏㅂㅅ'))).toBe('값');
  expect(typed(letters('ㅆㅏ'))).toBe('싸');
});

test('Backspace takes back the last letter, as the iPhone does', () => {
  expect(typed([...letters('ㄱㅏㅂ'), '\x7f'])).toBe('가');
  expect(typed([...letters('ㄱㅏㅂ'), '\x7f', '\x7f'])).toBe('ㄱ');
  expect(typed([...letters('ㄱㅏㅂ'), '\x7f', '\x7f', '\x7f'])).toBe('');
  expect(typed([...letters('ㄱㅏ'), '\x7f', 'ㅗ'])).toBe('고');
});

test('anything else passes untouched and ends the syllable', () => {
  const c = new HangulComposer();
  expect(c.feed('ㄱ')).toBe('ㄱ');
  expect(c.feed('l')).toBe('l');
  expect(c.feed('ㅏ')).toBe('ㅏ');
  expect(c.feed('\r')).toBe('\r');
  expect(c.feed('\x7f')).toBe('\x7f');
  expect(c.feed('한글')).toBe('한글');
  expect(typed(['ㅎ', 'ㅏ', '\x1b[D', 'ㄴ'])).toBe('하\x1b[Dㄴ');
});

test('only on an iPhone or iPad', () => {
  expect(needsHangulComposer({ userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) CriOS/130', maxTouchPoints: 5 })).toBe(true);
  expect(needsHangulComposer({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Safari', maxTouchPoints: 5 })).toBe(true);
  expect(needsHangulComposer({ userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome', maxTouchPoints: 0 })).toBe(false);
  expect(needsHangulComposer({ userAgent: 'Mozilla/5.0 (Windows NT 10.0) Chrome', maxTouchPoints: 0 })).toBe(false);
  expect(needsHangulComposer({ userAgent: 'Mozilla/5.0 (Linux; Android 14) Chrome Mobile', maxTouchPoints: 5 })).toBe(false);
});

import { charKeysym, inputDiff } from './consoleKeys';

describe('inputDiff', () => {
  it('sends what was typed at the end', () => {
    expect(inputDiff('ab', 'abc')).toEqual({ back: 0, add: 'c' });
  });

  it('takes back a syllable a Korean keyboard is still building', () => {
    expect(inputDiff('ㄱ', '가')).toEqual({ back: 1, add: '가' });
    expect(inputDiff('한가', '한각')).toEqual({ back: 1, add: '각' });
  });

  it('splits a syllable when the next vowel takes its last consonant', () => {
    expect(inputDiff('각', '가가')).toEqual({ back: 1, add: '가가' });
  });

  it('sends Backspace for a deletion and nothing for no change', () => {
    expect(inputDiff('abc', 'ab')).toEqual({ back: 1, add: '' });
    expect(inputDiff('abc', 'abc')).toEqual({ back: 0, add: '' });
  });

  it('counts characters, not UTF-16 units', () => {
    expect(inputDiff('a😀', 'a')).toEqual({ back: 1, add: '' });
  });
});

describe('charKeysym', () => {
  it('keeps Latin-1 and maps the rest to Unicode keysyms', () => {
    expect(charKeysym(0x61)).toBe(0x61);
    expect(charKeysym(0xac00)).toBe(0x0100ac00);
  });
});

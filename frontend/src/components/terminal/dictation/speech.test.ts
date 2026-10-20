import { finalText, Resampler, Typist } from './speech';

test('words are typed once, as they come', () => {
  const t = new Typist();
  expect(t.to('안녕')).toEqual([{ type: 'text', text: '안녕' }]);
  expect(t.to('안녕')).toEqual([]);
  expect(t.to('안녕 하세요')).toEqual([{ type: 'text', text: ' 하세요' }]);
  expect(t.pending('안녕 하세요 반가워')).toBe('반가워');
});

test('a spoken Enter presses Enter instead of being typed', () => {
  const t = new Typist();
  expect(t.to('ls -al 엔터.')).toEqual([{ type: 'text', text: 'ls -al' }, { type: 'enter' }]);
  // What follows starts at the new prompt, without its space.
  expect(t.to('ls -al 엔터. pwd Enter')).toEqual([{ type: 'text', text: 'pwd' }, { type: 'enter' }]);
  expect(new Typist().to('엔터키')).toEqual([{ type: 'enter' }]);
  expect(new Typist().to('ls 엔터 pwd 엔터')).toEqual([
    { type: 'text', text: 'ls' }, { type: 'enter' }, { type: 'text', text: 'pwd' }, { type: 'enter' },
  ]);
  // Only the word on its own: not inside another word.
  expect(new Typist().to('엔터테인먼트 뉴스')).toEqual([{ type: 'text', text: '엔터테인먼트 뉴스' }]);
});

test('a correction on the current line erases and retypes', () => {
  const t = new Typist();
  t.to('오늘 날씨.');
  expect(t.to('오늘 날씨가 좋습니다.')).toEqual([
    { type: 'erase', count: 1 }, { type: 'text', text: '가 좋습니다.' },
  ]);
  // Korean characters are erased one key each.
  expect(t.to('오늘 날씨가 맑습니다.')).toEqual([{ type: 'erase', count: 5 }, { type: 'text', text: '맑습니다.' }]);
});

test('a line that ran is never touched again', () => {
  const t = new Typist();
  t.to('ls 엔터 cd');
  // The service now hears the first command differently: it already ran.
  expect(t.to('ls. 엔터 cd /tmp')).toEqual([{ type: 'text', text: ' /tmp' }]);
  // On the current line corrections still work, after the leading space that was never typed.
  expect(t.to('ls 엔터 cp')).toEqual([{ type: 'erase', count: 6 }, { type: 'text', text: 'p' }]);
});

test('the final text joins what was committed and the last partial', () => {
  expect(finalText('ls', '-al')).toBe('ls -al');
  expect(finalText('ls ', '-al')).toBe('ls -al');
  expect(finalText('', 'pwd')).toBe('pwd');
  expect(finalText('pwd', '')).toBe('pwd');
});

test('48 kHz becomes 16 kHz in 0.1 s frames', () => {
  const r = new Resampler(48000);
  expect(r.push(new Float32Array(4799))).toHaveLength(0);
  const silent = r.push(new Float32Array(1));
  expect(silent).toHaveLength(1);
  expect(silent[0].byteLength).toBe(3200);
  const loud = r.push(new Float32Array(4800).fill(0.5));
  expect(loud).toHaveLength(1);
  expect(new Int16Array(loud[0]).every((v) => v === 16383)).toBe(true);
  // Clipped, never wrapped.
  const clipped = new Resampler(16000).push(new Float32Array(1600).fill(2));
  expect(new Int16Array(clipped[0])[0]).toBe(32767);
});

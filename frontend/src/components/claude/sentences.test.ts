import { Sentences, speakable } from './sentences';

test('sentences are cut as they end, while the answer streams', () => {
  const s = new Sentences();
  expect(s.push('안녕하세요')).toEqual([]);
  expect(s.push(', 나비 님. 하늘이 파란 ')).toEqual(['안녕하세요, 나비 님.']);
  expect(s.push('까닭은 빛의 산란 때문입니다. 네')).toEqual(['하늘이 파란 까닭은 빛의 산란 때문입니다.']);
  // A short one waits for the next…
  expect(s.push('. 그리고')).toEqual([]);
  expect(s.push(' 노을은 붉게 보이지요! ')).toEqual(['네. 그리고 노을은 붉게 보이지요!']);
  expect(s.flush()).toEqual([]);
  s.push('끝맺음이 없는 말');
  expect(s.flush()).toEqual(['끝맺음이 없는 말']);
});

test('a number with a dot is not a sentence end', () => {
  const s = new Sentences();
  expect(s.push('원주율은 3.14로 시작합니다. ')).toEqual(['원주율은 3.14로 시작합니다.']);
});

test('a long run with no end is cut at a pause', () => {
  const s = new Sentences();
  const out = s.push(`${'가나다라 '.repeat(60)}`);
  expect(out.length).toBe(1);
  expect(out[0].length).toBeLessThanOrEqual(220);
});

test('what only makes sense on screen is not read', () => {
  expect(speakable('## 정리\n- **첫째**, `ls -l`을 보세요.\n1. [문서](https://x.y/z) 참고')).toBe('정리 첫째, ls -l을 보세요. 문서 참고');
  expect(speakable('```bash\nrm -rf /\n```')).toBe('');
  expect(speakable('snake_case_name 그대로')).toBe('snake_case_name 그대로');
});

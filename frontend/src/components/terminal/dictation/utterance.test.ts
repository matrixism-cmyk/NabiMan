import { utteranceActions } from './speech';

test('a finished utterance is typed, and "엔터" presses Enter where it was said', () => {
  expect(utteranceActions('ls -al 엔터', false, false)).toEqual([{ type: 'text', text: 'ls -al' }, { type: 'enter' }]);
  expect(utteranceActions('cd /tmp 엔터 pwd 엔터.', false, false)).toEqual([
    { type: 'text', text: 'cd /tmp' }, { type: 'enter' }, { type: 'text', text: 'pwd' }, { type: 'enter' },
  ]);
});

test('without "엔터" the words wait on the line, set apart from what is there', () => {
  expect(utteranceActions('git status', false, false)).toEqual([{ type: 'text', text: 'git status' }]);
  expect(utteranceActions('--short', false, true)).toEqual([{ type: 'text', text: ' --short' }]);
});

test('sending on a pause presses Enter after the words, but never on nothing', () => {
  expect(utteranceActions('사과는 무슨 색이야', true, false)).toEqual([{ type: 'text', text: '사과는 무슨 색이야' }, { type: 'enter' }]);
  expect(utteranceActions('질문이야 엔터', true, false)).toEqual([{ type: 'text', text: '질문이야' }, { type: 'enter' }]);
  expect(utteranceActions('  ', true, false)).toEqual([]);
});

test('"엔터" inside a word is just a word', () => {
  expect(utteranceActions('엔터프라이즈 서버', false, false)).toEqual([{ type: 'text', text: '엔터프라이즈 서버' }]);
});

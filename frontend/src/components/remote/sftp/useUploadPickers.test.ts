import { fromPicked } from './useUploadPickers';

const file = (name: string, rel = '') => {
  const f = new File(['x'], name);
  if (rel) Object.defineProperty(f, 'webkitRelativePath', { value: rel });
  return f;
};

test('a folder pick keeps paths and recreates subfolders, parents first', () => {
  const out = fromPicked([file('a.txt', 'proj/a.txt'), file('b.c', 'proj/src/lib/b.c'), file('m.h', 'proj/src/m.h')]);
  expect(out.files.map((f) => f.rel)).toEqual(['proj/a.txt', 'proj/src/lib/b.c', 'proj/src/m.h']);
  expect(out.dirs).toEqual(['proj', 'proj/src', 'proj/src/lib']);
});

test('a plain file pick has no folders', () => {
  const out = fromPicked([file('one.txt'), file('two.txt')]);
  expect(out.files.map((f) => f.rel)).toEqual(['one.txt', 'two.txt']);
  expect(out.dirs).toEqual([]);
});

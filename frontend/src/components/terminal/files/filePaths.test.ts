import { classify, findPaths } from './filePaths';

const paths = (text: string) => findPaths(text).map((m) => m.path);
const first = (text: string) => { const m = findPaths(text)[0]; return m ? text.slice(m.start, m.end) : null; };

describe('paths in terminal text', () => {
  it('finds the file in a Claude Code file card, spaces and all', () => {
    const line = '› [file] 20261125_v3_V-Nexus2026_강의자료.pdf (678.4KB)';
    expect(paths(line)).toEqual(['20261125_v3_V-Nexus2026_강의자료.pdf']);
    expect(first(line)).toBe('20261125_v3_V-Nexus2026_강의자료.pdf');
    expect(paths('[file] 최종 보고서 v2.hwp (1.2 MB)')[0]).toBe('최종 보고서 v2.hwp');
    expect(paths('[file] notes (final) (12 B)')[0]).toBe('notes (final)');
  });

  it('finds an absolute path with Korean folders in running text', () => {
    const path = '/workspace/5gmec/작업본/_전북진흥원_강의_20261203/deck/20261125_v3_V-Nexus2026_강의자료.pptx';
    expect(paths(`발표 자료는 ${path} 에 있습니다.`)).toEqual([path]);
    expect(findPaths(`saved ${path}`)[0]).toMatchObject({ kind: 'absolute', start: 6, end: 6 + path.length });
  });

  it('finds home, relative and bare names', () => {
    expect(findPaths('see ~/notes/todo.md')[0]).toMatchObject({ path: '~/notes/todo.md', kind: 'home' });
    expect(findPaths('edit ./run.sh now')[0]).toMatchObject({ path: './run.sh', kind: 'relative' });
    expect(findPaths('in frontend/src/App.tsx')[0]).toMatchObject({ path: 'frontend/src/App.tsx', kind: 'relative' });
    expect(findPaths('wrote 강의자료.pptx')[0]).toMatchObject({ path: '강의자료.pptx', kind: 'name' });
    expect(paths('cat .env README.md Makefile')).toEqual(['.env', 'README.md']);
  });

  it('ends a path at a space unless it is quoted', () => {
    expect(paths('open /tmp/my file.txt')).toEqual(['/tmp/my', 'file.txt']);
    expect(paths('open "/tmp/my file.txt" now')[0]).toBe('/tmp/my file.txt');
    expect(paths("saved 'final report.pdf'")[0]).toBe('final report.pdf');
    // The longer reading comes first; the shorter ones stay for when it is not a file.
    expect(paths('"see report.pdf"')).toEqual(['see report.pdf', 'report.pdf']);
  });

  it('leaves punctuation and Korean particles around a path out of it', () => {
    expect(paths('(see /etc/nginx/nginx.conf).')).toEqual(['/etc/nginx/nginx.conf']);
    expect(paths('Saved to /tmp/out.csv, then /tmp/log.txt: done')).toEqual(['/tmp/out.csv', '/tmp/log.txt']);
    expect(paths('**src/main.rs**')).toEqual(['src/main.rs']);
    expect(paths('파일은 /tmp/결과.xlsx입니다.')).toEqual(['/tmp/결과.xlsx']);
    expect(paths('report.pdf를 열어 주세요')).toEqual(['report.pdf']);
    expect(paths('--out=/tmp/x.log')).toEqual(['/tmp/x.log']);
    expect(paths('got report(1).pdf)')).toEqual(['report(1).pdf']);
    expect(paths('│ /tmp/a.txt │')).toEqual(['/tmp/a.txt']);
  });

  it('strips a line and column suffix', () => {
    expect(paths('src/app.ts:12')).toEqual(['src/app.ts']);
    expect(paths('src/app.ts:12:5: error TS2304')).toEqual(['src/app.ts']);
    expect(paths('/a/b/c.py:7,')).toEqual(['/a/b/c.py']);
    const m = findPaths('at src/app.ts:12:5')[0];
    expect('at src/app.ts:12:5'.slice(m.start, m.end)).toBe('src/app.ts');
  });

  it('does not take versions, addresses, hosts or abbreviations for files', () => {
    for (const text of [
      'claude 2.1.112 is out', '10.0.0.12', '192.168.0.1:8080', 'https://example.com/a/b.pdf', 'http://x.kr/file.zip',
      'github.com', 'v.bebo.kr', 'the@aeokorea.com', 'e.g. this', 'i.e. that', 'and/or', 'TCP/IP', '2026/11/25',
      '/help', '/compact', '// a comment', 's/foo/bar/', '--flag.json=1', 'example.com/a/b.pdf', '$HOME/x.txt',
      '3.5KB', 'No.1', 'a - b', '...', '</div>', 'obj.method()', 'Mr.Kim', 'C:\\Users\\x.txt',
    ]) {
      expect({ text, found: paths(text) }).toEqual({ text, found: [] });
    }
  });

  it('classifies what a card or quotes vouch for', () => {
    expect(classify('notes', 'card')).toBe('name');
    expect(classify('notes')).toBeNull();
    expect(classify('my file.txt')).toBeNull();
    expect(classify('my file.txt', 'quoted')).toBe('name');
    expect(classify('/x')).toBeNull();
    expect(classify('/x.log')).toBe('absolute');
    expect(classify(`/${'a'.repeat(2000)}/b`)).toBeNull();
  });
});

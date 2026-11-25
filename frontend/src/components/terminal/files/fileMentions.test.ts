import { Terminal } from 'xterm';
import { mentionedPaths } from './fileMentions';

async function screen(text: string, cols = 60, rows = 12): Promise<Terminal> {
  const term = new Terminal({ cols, rows, allowProposedApi: true, scrollback: 500 });
  await new Promise<void>((done) => term.write(text, done));
  return term;
}

describe('a name whose full path was printed elsewhere', () => {
  it('finds the path that ends in the name, wrapped or not, nearest first', async () => {
    const deep = '/workspace/5gmec/작업본/_전북진흥원_강의_20261203/deck/V-Nexus2026_강의자료_20261125_v5.pdf';
    const term = await screen(`old /tmp/a/V-Nexus2026_강의자료_20261125_v5.pdf\r\nWrote ${deep}\r\n\r\n› [file] V-Nexus2026_강의자료_20261125_v5.pdf (678.4KB)\r\n`);
    expect(mentionedPaths(term, 'V-Nexus2026_강의자료_20261125_v5.pdf')).toEqual([deep, '/tmp/a/V-Nexus2026_강의자료_20261125_v5.pdf']);
  });

  it('is not fooled by a longer name that merely ends the same, nor by a bare name', async () => {
    const term = await screen('see /tmp/my-report.pdf and report.pdf and ~/docs/report.pdf\r\n');
    expect(mentionedPaths(term, 'report.pdf')).toEqual(['~/docs/report.pdf']);
  });

  it('gives nothing for a name never printed with a folder, or too short to mean anything', async () => {
    const term = await screen('report.pdf\r\n/tmp/a.c\r\n');
    expect(mentionedPaths(term, 'report.pdf')).toEqual([]);
    expect(mentionedPaths(term, 'a')).toEqual([]);
  });
});

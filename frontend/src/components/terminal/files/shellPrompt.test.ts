import { Terminal } from 'xterm';
import { MAX_ROWS_UP, promptFolder, readPrompt } from './shellPrompt';

/** A real xterm (never opened) with `text` written to it. */
async function screen(text: string, cols = 60, rows = 12, scrollback = 1000): Promise<Terminal> {
  const term = new Terminal({ cols, rows, scrollback, allowProposedApi: true });
  await new Promise<void>((done) => term.write(text, done));
  return term;
}
const dir = (line: string) => readPrompt(line)?.dir;

describe('a shell prompt, read for its folder', () => {
  it('reads the common forms', () => {
    expect(dir('nabi@box:/srv/app$ ')).toBe('/srv/app');
    expect(dir('root@box:~/sub# ')).toBe('~/sub');
    expect(dir('nabi@box:~$ ')).toBe('~');
    expect(dir('nabi@box:~$')).toBe('~'); // the cursor right after it, nothing typed
    expect(dir('root@mec-01.example.com:/# ')).toBe('/');
    expect(dir('nabi@box:/data/한글 폴더$ ls')).toBe('/data/한글 폴더');
    expect(dir('(venv) nabi@box:~/proj$ ')).toBe('~/proj');
    expect(dir('(base) (venv) nabi@box:/w$ ')).toBe('/w');
    expect(dir('nabi@box:~/proj (main)$ git status')).toBe('~/proj');
    expect(dir('nabi@box:~/proj% ')).toBe('~/proj');
    // fish's default, and a zsh prompt of the same shape.
    expect(dir('nabi@box ~/proj> ')).toBe('~/proj');
    expect(dir('root@box /etc/nginx# ')).toBe('/etc/nginx');
    expect(dir('nabi@box /srv/app % ls')).toBe('/srv/app');
    // Bracketed, when it shows a path.
    expect(dir('[root@centos ~]# ')).toBe('~');
    expect(dir('[nabi@centos /var/log]$ tail x.log')).toBe('/var/log');
  });

  it('reads the folder, not the command typed after it', () => {
    expect(dir('nabi@box:/srv/app$ echo $HOME # not a folder')).toBe('/srv/app');
    expect(dir('nabi@box:~$ scp a.txt root@other:/tmp/x$ ')).toBe('~');
    expect(dir('nabi@box:/a$ cd /b && echo "c@d:/e$ "')).toBe('/a');
  });

  it('finds a prompt that names no usable folder, and says so', () => {
    expect(readPrompt('[nabi@centos app]$ ls')).toEqual({ dir: null });
    expect(readPrompt('nabi@box:app$ ls')).toEqual({ dir: null });
    expect(readPrompt('nabi@box:~other/x$ ')).toEqual({ dir: null });
  });

  it('is not fooled by output that merely holds : and $ or looks a little like a prompt', () => {
    for (const line of [
      '', 'total 12', 'Price: $5 only', 'PATH=/usr/bin:/bin $HOME', 'note.txt', 'key: value # comment',
      'error: cannot open /tmp/x: $ARG unset', 'git@github.com:org/repo.git', 'From git@github.com:org/repo',
      'root@box:/remote/file.txt                100%   12KB  1.2MB/s', 'nabi@box:/f 100% done',
      'Connecting to nabi@box:/srv ...', '  echo nabi@box:/srv$ ', 'tmpfs /run 1% ', 'box% ls', '➜  app git:(main) ls',
      '/srv/app $ ls', 'PS C:\\Users\\x> dir', 'mail nabi@box /tmp/x', '$ nabi@box:/srv$ ', 'user@host:$ ',
    ]) expect(readPrompt(line)).toBeNull();
  });
});

describe('the nearest prompt above a line on screen', () => {
  it('is the one the output was printed under, not a later or an earlier one', async () => {
    const term = await screen([
      'nabi@box:~$ cd /srv/app/sub', // 0
      'nabi@box:/srv/app/sub$ echo note.txt', // 1
      'note.txt', // 2
      'nabi@box:/srv/app/sub$ cd /', // 3
      'nabi@box:/$ echo note.txt', // 4
      'note.txt', // 5
      'nabi@box:/$ ', // 6
    ].join('\r\n'));
    expect(promptFolder(term, 2)).toBe('/srv/app/sub');
    expect(promptFolder(term, 5)).toBe('/');
    expect(promptFolder(term, 0)).toBe('~');
    // A name on the prompt's own line (an argument typed there) is under that prompt.
    expect(promptFolder(term, 1)).toBe('/srv/app/sub');
  });

  it('reads a prompt and command the terminal wrapped, from any of its rows', async () => {
    const term = await screen('nabi@box:/srv/app/a-rather-long-folder-name$ cat one.txt two.txt three.txt\r\nout.txt', 30);
    expect(term.buffer.active.getLine(1)?.isWrapped).toBe(true);
    for (const y of [0, 1, 2, 3]) expect(promptFolder(term, y)).toBe('/srv/app/a-rather-long-folder-name');
  });

  it('reads a prompt a screen-drawing program cut at the edge', async () => {
    // tmux ends the row itself: no wrap mark, the row is just full.
    const term = await screen('nabi@box:/srv/app/a-rather-lon\r\ng-folder-name$ ls\r\nout.txt', 30);
    expect(term.buffer.active.getLine(1)?.isWrapped).toBe(false);
    expect(promptFolder(term, 2)).toBe('/srv/app/a-rather-long-folder-name');
  });

  it('gives nothing without a prompt, under one that shows no folder, or beyond its reach', async () => {
    expect(promptFolder(await screen('total 4\r\nnote.txt\r\nPrice: $5'), 2)).toBeNull();
    // The CentOS prompt hides an older one that showed a path: that one is stale.
    expect(promptFolder(await screen('nabi@box:/old$ su -\r\n[root@box app]# ls\r\nnote.txt'), 2)).toBeNull();
    const far = `nabi@box:/srv$ seq\r\n${Array.from({ length: MAX_ROWS_UP + 5 }, (_, i) => String(i)).join('\r\n')}`;
    const term = await screen(far, 60, 12, 1000);
    const last = term.buffer.active.length - 1;
    expect(promptFolder(term, last)).toBeNull();
    expect(promptFolder(term, MAX_ROWS_UP - 1)).toBe('/srv');
    expect(promptFolder(term, 99999)).toBeNull();
    expect(promptFolder(term, -1)).toBeNull();
  });
});

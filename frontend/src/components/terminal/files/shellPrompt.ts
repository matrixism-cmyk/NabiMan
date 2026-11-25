import type { IBuffer, Terminal } from 'xterm';
import { continues } from './fileLines';

/**
 * The folder a shell prompt on a terminal's screen shows.
 *
 * A name printed by `ls` or `echo` is in the folder the shell was in when it
 * ran, and the prompt above the output says which. That works wherever the
 * shell is — this machine or one reached over SSH — and for output further
 * up, printed before a later `cd`.
 *
 * Recognised, at the start of a line (after `(venv)`-like marks):
 *   user@host:/abs/path$    user@host:~/sub#    user@host:~$     (bash, Debian's default; `%` too)
 *   user@host /abs/path>    user@host ~/sub %                    (fish's default, a zsh `%n@%m %~ %#`)
 *   [user@host ~]$          [user@host /abs/path]#               (bracketed, when it shows a path)
 * A prompt that shows only the folder's own name (`[user@host dir]$`, CentOS's
 * default, or `user@host:dir$`) names no folder: the search ends there with
 * nothing rather than going on to an older prompt. Prompts without
 * `user@host` (`host% `, `➜ dir`, `PS C:\>`, `/path $`) are not recognised:
 * too much ordinary output looks like them.
 *
 * What a screen shows is not trusted. The folder is only ever asked about
 * through the file panel's own listing request, which checks the account.
 */

/** Rows looked through at most, upward from the one asked about. */
export const MAX_ROWS_UP = 200;
/** Rows a prompt and its path may run over. */
const MAX_JOINED = 4;
const MAX_DIR = 1024;

const MARKS = '(?:\\([^()\\s]{1,40}\\)\\s?)*';
const WHO = '[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}@[A-Za-z0-9][A-Za-z0-9_.-]{0,253}';
/** A folder with a space in it is read only before `$` or `#`: `100% ` ends too much plain output. */
const COLON = new RegExp(`^\\s?${MARKS}${WHO}:(?:((?:~|/)\\S{0,${MAX_DIR}}?)\\s?[$#%]|((?:~|/).{0,${MAX_DIR}}?)\\s?[$#])(?:\\s|$)`);
const SPACED = new RegExp(`^\\s?${MARKS}${WHO} ((?:~|/)\\S{0,${MAX_DIR}}?)\\s?[$#%>](?:\\s|$)`);
const BRACKETED = new RegExp(`^\\s?${MARKS}\\[${WHO}[ :]([^\\]]{1,${MAX_DIR}})\\]\\s?[$#%](?:\\s|$)`);
/** `user@host:dir$`: a prompt, but of the folder's own name only. */
const NAME_ONLY = new RegExp(`^\\s?${MARKS}${WHO}:[^\\s/~:][^\\s:]{0,255}[$#](?:\\s|$)`);

export interface Prompt {
  /** The folder as shown: `/abs/path`, `~` or `~/sub`; null when the prompt shows none that can be used. */
  dir: string | null;
}

/** `~`, `~/sub` or an absolute path; not `~user`, not a bare name. */
function usable(shown: string): string | null {
  // A git branch or the like after the folder: `~/proj (main)`.
  const dir = shown.trim().replace(/\s+[([][^()[\]]*[)\]]$/, '');
  return dir === '~' || dir.startsWith('~/') || dir.startsWith('/') ? dir : null;
}

/** The prompt `line` starts with, or null when it starts with none. */
export function readPrompt(line: string): Prompt | null {
  const bracketed = BRACKETED.exec(line);
  if (bracketed) return { dir: usable(bracketed[1]) };
  const shown = COLON.exec(line) || SPACED.exec(line);
  if (shown) return { dir: usable(shown[1] ?? shown[2]) };
  return NAME_ONLY.test(line) ? { dir: null } : null;
}

/** Row `top` together with the rows that carry on from it. */
function joined(buffer: IBuffer, cols: number, top: number, loose: boolean): string {
  let text = '';
  for (let y = top; y < top + MAX_JOINED; y++) {
    const line = buffer.getLine(y);
    if (!line) break;
    const more = y + 1 < top + MAX_JOINED && continues(buffer, cols, y + 1, loose);
    // A row the terminal wrapped is whole to its edge, spaces included.
    text += line.translateToString(!(more && buffer.getLine(y + 1)?.isWrapped), 0, cols);
    if (!more) break;
  }
  return text;
}

/**
 * The folder shown by the nearest prompt at or above buffer row `y` (from 0):
 * `/abs/path`, `~` or `~/sub`. Null when there is no prompt within reach, or
 * the nearest one shows no folder.
 */
export function promptFolder(term: Terminal, y: number): string | null {
  const buffer = term.buffer.active;
  const cols = term.cols;
  let row = Math.min(y, buffer.length - 1);
  for (let seen = 0; row >= 0 && seen < MAX_ROWS_UP; seen++, row--) {
    // A row the terminal wrapped is read with the row it started on.
    if (buffer.getLine(row)?.isWrapped) continue;
    // A program that draws its own screen (tmux) ends a long line itself: a
    // prompt cut at the edge is read on into the next row.
    const prompt = readPrompt(joined(buffer, cols, row, false)) || readPrompt(joined(buffer, cols, row, true));
    if (prompt) return prompt.dir;
  }
  return null;
}

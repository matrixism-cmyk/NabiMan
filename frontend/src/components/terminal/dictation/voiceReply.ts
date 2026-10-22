/**
 * "음성으로 대답해줘" said at the end of a dictated line: the words are not
 * typed, and once the line has run and its answer is on screen, the last
 * answer is read aloud. These are the pieces that need no browser: finding
 * the request in what was said, and the answer in what the terminal shows.
 */

/** The request, at the end of a line (or just before a spoken "엔터"). */
const ASK = new RegExp(
  String.raw`\s*(?:(?:음성|목소리|소리|말)\s*(?:으로|로)\s*(?:대답|답변|답|알려|말|설명|읽어)\s*(?:을|를)?\s*(?:해|하여)?\s*`
  + String.raw`(?:줘요?|주세요|줄래요?|주라|주십시오)|(?:대답|답변|결과|답)\s*(?:을|를)?\s*(?:소리\s*내어\s*)?읽어\s*(?:줘요?|주세요|줄래요?))`
  + String.raw`[.,!?。、…~\s]*(?=(?:엔터키?|enter)(?:[\s.,!?]|$)|$)`,
  'giu',
);

/** `text` without the request, and how many times it was made. */
export function takeVoiceAsk(text: string): { text: string; asks: number } {
  let asks = 0;
  const rest = text.replace(ASK, (found, offset: number, whole: string) => {
    asks += 1;
    // Keep a space where the request stood between words, so "엔터" still stands alone.
    return offset > 0 && offset + found.length < whole.length ? ' ' : '';
  });
  return { text: rest, asks };
}

// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]/g;
const RULE = /^[─━-]{10,}$/;
const BULLET = /^[●⏺]\s/;
/** A tool at work rather than words: "● Bash(ls)", "● Read(file)", "● Monitor …". */
const TOOL = /^[●⏺]\s+(?:[A-Z][\w-]*\(|(?:Bash|Read|Write|Edit|Update|Search|Fetch|Task|Agent|Monitor|Glob|Grep)\b)/;
/** Claude Code's status lines: "✻ Churned for 2s", "✽ Thinking…". */
const STATUS = /^[✻✽✢✳✶·*]\s/;
/** A shell prompt with a command after it: "user@host:~$ ls". */
const SHELL_PROMPT = /[$#%❯>]\s+\S.*$/;

/** The last answer on a terminal's screen, as plain text to read aloud; '' when there is none. */
export function lastAnswer(screen: string): string {
  const lines = screen.replace(ANSI, '').split('\n').map((l) => l.replace(/\s+$/, ''));
  while (lines.length && !lines[lines.length - 1].trim()) lines.pop();
  const rules = lines.map((l, i) => (RULE.test(l.trim()) ? i : -1)).filter((i) => i >= 0);
  // Claude Code: the transcript ends where its input box (between two rules) begins.
  const boxTop = rules.length >= 2 ? rules[rules.length - 2] : rules.length === 1 ? rules[0] : lines.length;
  const transcript = lines.slice(0, boxTop);
  let asked = -1;
  transcript.forEach((l, i) => { if (/^[❯>]\s+\S/.test(l)) asked = i; });
  const reply = transcript.slice(asked + 1);
  if (reply.some((l) => BULLET.test(l))) return claudeAnswer(reply);
  return shellOutput(lines);
}

function claudeAnswer(reply: string[]): string {
  const blocks: string[][] = [];
  let block: string[] | null = null;
  for (const line of reply) {
    if (BULLET.test(line)) {
      block = TOOL.test(line) ? null : [line.replace(BULLET, '').trim()];
      if (block) blocks.push(block);
    } else if (block && /^ {2}/.test(line) && !/^\s*⎿/.test(line)) {
      block.push(line.trim());
    } else if (block && !line.trim()) {
      block.push('');
    } else {
      block = STATUS.test(line) || line.trim() ? null : block;
    }
  }
  const last = blocks[blocks.length - 1] || [];
  return tidy(last.join('\n'));
}

/** A shell: what the last command printed, between its prompt and the one now waiting. */
function shellOutput(lines: string[]): string {
  const end = lines.length - 1;
  let start = end - 1;
  while (start >= 0 && !SHELL_PROMPT.test(lines[start]) && end - start < 40) start -= 1;
  return tidy(lines.slice(start + 1, end).join('\n'));
}

/** Text fit for reading: no drawing characters or markdown marks, a sensible length. */
export function tidy(text: string, max = 1500): string {
  const clean = text
    .replace(/[│┃┆┊╎╭╮╰╯├┤┬┴┼─━═]+/g, ' ')
    .replace(/```[\s\S]*?```/g, ' 코드 생략. ')
    .replace(/\*\*|__|`/g, '')
    .replace(/^\s*[-*•]\s+/gm, '')
    .replace(/[ \t]+/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
  if (clean.length <= max) return clean;
  const cut = clean.slice(0, max);
  const stop = Math.max(cut.lastIndexOf('. '), cut.lastIndexOf('다.'), cut.lastIndexOf('\n'));
  return `${cut.slice(0, stop > max / 2 ? stop + 1 : max)} 이하 생략.`;
}

/** Sentences of at most `max` characters, for the speech service one at a time. */
export function sentences(text: string, max = 280): string[] {
  const out: string[] = [];
  for (const part of text.split(/(?<=[.!?。]|다\.|요\.)\s+|\n+/u)) {
    let rest = part.trim();
    while (rest.length > max) {
      const at = rest.lastIndexOf(' ', max);
      out.push(rest.slice(0, at > max / 2 ? at : max));
      rest = rest.slice(at > max / 2 ? at + 1 : max).trim();
    }
    if (rest) out.push(rest);
  }
  return out;
}

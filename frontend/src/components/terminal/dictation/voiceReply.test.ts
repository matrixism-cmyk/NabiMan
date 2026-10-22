import { lastAnswer, sentences, takeVoiceAsk, tidy } from './voiceReply';

/** Claude Code in tmux, as capture-pane -e -J gives it: colours, a tool call, the input box. */
const CLAUDE = "\u001b[38;5;174m ▐\u001b[48;5;16m▛███▛█\u001b[39m\u001b[49m   \u001b[1mClaude Code\u001b[0m \u001b[38;5;246mv2.1.308\n\u001b[38;5;174m▝▜\u001b[48;5;16m█████\u001b[49m█▀\u001b[39m  \u001b[38;5;246mOpus 5.5 with high effort · Claude Max\n\u001b[38;5;174m ▝▝   ▝▝ \u001b[39m  \u001b[38;5;246m/tmp/claude-1000/-workspace-nabiman/b04ff501-7e25-4c98-bd38-f9ff77d5d303/scratchpad/ccprobe\n\n\n\u001b[38;5;114m●\u001b[39m Monitor \"fixhub: new reports, admin answers and chat\" ended without producing output (exit 0)\n\n\u001b[38;5;231m●\u001b[39m The background monitor for fixhub (new reports, admin answers and chat) has stopped. It exited cleanly and\n  saw no new reports, admin answers or chat messages while it ran. Nothing needs action from it. I can restart\n  the watch if you want it to keep running.\n\n\u001b[38;5;246m✻\u001b[39m \u001b[38;5;246mCooked for 3s · done 오후 6:09\n\n\u001b[48;5;237m❯ \u001b[38;5;231m대한민국의 수도와 일본의 수도를 각각 한 문장으로 알려줘. 도구는 쓰지 마.\u001b[39m                                    \n\n\u001b[38;5;231m\u001b[49m●\u001b[39m 대한민국의 수도는 서울입니다.\n  일본의 수도는 도쿄입니다.\n\n\u001b[38;5;246m✻\u001b[39m \u001b[38;5;246mChurned for 2s · done 오후 6:09\n\n\u001b[48;5;237m❯ \u001b[38;5;231m/tmp 폴더에 파일이 몇 개 있는지 ls로 확인하고 한 문장으로 알려줘.\u001b[39m                                           \n\n\u001b[49m  \u001b[38;5;246mListed \u001b[1m1\u001b[0m\u001b[38;5;246m directory \u001b[39m     \n\n\u001b[38;5;231m●\u001b[39m \u001b[38;5;153mls -A\u001b[39m로 세어 보니 /tmp에는 숨김 항목과 하위 폴더까지 포함해 모두 9,358개의 항목이 있습니다.\n\n\u001b[38;5;246m✻\u001b[39m \u001b[38;5;246mCooked for 5s · done 오후 6:11\n\n\n\n\n\n\n\u001b[39m                                                                                                            \n\u001b[38;5;244m──────────────────────────────────────────────────────────────────────────────────────────────────────────────\n\u001b[39m❯                                                                         \n\u001b[38;5;244m──────────────────────────────────────────────────────────────────────────────────────────────────────────────\n\u001b[39m  \u001b[38;5;220m⚠ Transcript saving is off — inherited CLAUDE_CODE_CHILD_SESSION marker\u001b[38;5;246m · restart with CLAUDE_CODE_FORCE_…\n\u001b[39m  \u001b[38;5;220m⏵⏵ auto mode on\u001b[38;5;246m (shift+tab to cycle)\u001b[39m                   \n";

describe('asking for a spoken answer', () => {
  it('takes the request off the end of what was said', () => {
    expect(takeVoiceAsk('서울 날씨 알려줘 음성으로 대답해줘')).toEqual({ text: '서울 날씨 알려줘', asks: 1 });
    expect(takeVoiceAsk('요약해 줘. 소리로 알려 주세요.')).toEqual({ text: '요약해 줘.', asks: 1 });
    expect(takeVoiceAsk('결과를 읽어줘')).toEqual({ text: '', asks: 1 });
    expect(takeVoiceAsk('목소리로 답해줘').asks).toBe(1);
  });
  it('before a spoken Enter too, which still stands alone', () => {
    expect(takeVoiceAsk('테스트 돌려 음성으로 대답해줘 엔터')).toEqual({ text: '테스트 돌려 엔터', asks: 1 });
  });
  it('leaves it alone in the middle of a sentence', () => {
    expect(takeVoiceAsk('음성으로 대답해줘 라는 기능을 만들자').asks).toBe(0);
    expect(takeVoiceAsk('그냥 대답해줘').asks).toBe(0);
  });
});

describe('the last answer on screen', () => {
  it("is Claude Code's last reply, without the tool lines, the status or the input box", () => {
    expect(lastAnswer(CLAUDE)).toBe('ls -A로 세어 보니 /tmp에는 숨김 항목과 하위 폴더까지 포함해 모두 9,358개의 항목이 있습니다.');
  });
  it('joins a reply of several lines and skips a tool call after it', () => {
    const screen = [
      '❯ 두 가지 알려줘', '● 첫째는 서울입니다.', '  둘째는 도쿄입니다.', '● Bash(ls /tmp)', '  ⎿  a b c',
      '✻ Churned for 2s · done', '─'.repeat(40), '❯ ', '─'.repeat(40), '  ⏵⏵ auto mode on',
    ].join('\n');
    expect(lastAnswer(screen)).toBe('첫째는 서울입니다.\n둘째는 도쿄입니다.');
  });
  it('is what a shell command printed, between its prompt and the next', () => {
    const screen = ['nabi@host:~$ echo old', 'old', 'nabi@host:~$ df -h /', 'Filesystem Size', '/dev/sda 100G', 'nabi@host:~$ '].join('\n');
    expect(lastAnswer(screen)).toBe('Filesystem Size\n/dev/sda 100G');
  });
  it('is nothing when nothing was answered', () => {
    expect(lastAnswer('')).toBe('');
  });
});

describe('reading it', () => {
  it('drops markdown marks, code and drawing characters, and stops at a length', () => {
    expect(tidy('**굵게** `코드` │ 표 │')).toBe('굵게 코드 표');
    expect(tidy('설명\n```\nrm -rf x\n```\n끝')).toBe('설명\n 코드 생략. \n끝');
    expect(tidy('가나다. '.repeat(400)).endsWith('이하 생략.')).toBe(true);
  });
  it('goes to the speech service a sentence at a time', () => {
    expect(sentences('서울입니다. 도쿄입니다.\n끝')).toEqual(['서울입니다.', '도쿄입니다.', '끝']);
    expect(sentences('가'.repeat(700)).every((s) => s.length <= 280)).toBe(true);
  });
});

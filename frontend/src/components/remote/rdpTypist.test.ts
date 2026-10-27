import { rdpTypist } from './rdpTypist';

function fakeClient() {
  const log: string[] = [];
  const stream = { sendBlob: (b64: string) => log.push(`clip:${Buffer.from(b64, 'base64').toString('utf8')}`), sendEnd: () => undefined };
  return {
    log,
    client: {
      createClipboardStream: () => stream,
      sendKeyEvent: (down: number, keysym: number) => log.push(`${keysym.toString(16)}${down ? '↓' : '↑'}`),
    },
  };
}

/** Lets every queued step and its waits run (this jest has no runAllTimersAsync). */
async function settle() {
  for (let i = 0; i < 20; i += 1) {
    jest.advanceTimersByTime(500);
    for (let j = 0; j < 5; j += 1) await Promise.resolve();
  }
}

describe('rdpTypist', () => {
  beforeEach(() => jest.useFakeTimers());
  afterEach(() => jest.useRealTimers());

  it('pastes the text, then presses the Enter that ended it', async () => {
    const { log, client } = fakeClient();
    const typist = rdpTypist(() => client);
    typist.text('hello 안녕');
    typist.keys('\r');
    await settle();
    expect(log).toEqual(['clip:hello 안녕', 'ffe3↓', '76↓', '76↑', 'ffe3↑', 'ff0d↓', 'ff0d↑']);
  });

  it('takes back characters with Backspace', async () => {
    const { log, client } = fakeClient();
    rdpTypist(() => client).keys('\x7f\x7f');
    await settle();
    expect(log).toEqual(['ff08↓', 'ff08↑', 'ff08↓', 'ff08↑']);
  });

  it('does nothing without a connection', async () => {
    const typist = rdpTypist(() => null);
    typist.text('x');
    await settle();
  });
});

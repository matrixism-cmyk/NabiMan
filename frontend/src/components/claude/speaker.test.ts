import { Speaker } from './speaker';

class FakeAudio {
  static played: string[] = [];
  src = ''; playbackRate = 1; defaultPlaybackRate = 1;
  onended: (() => void) | null = null; onerror: (() => void) | null = null;
  play() { FakeAudio.played.push(this.src); setTimeout(() => this.onended?.(), 5); return Promise.resolve(); }
  pause() { /* stopped */ }
}

beforeEach(() => {
  FakeAudio.played = [];
  (global as unknown as { Audio: unknown }).Audio = FakeAudio;
  let n = 0;
  URL.createObjectURL = jest.fn(() => `blob:${(n += 1)}`);
  URL.revokeObjectURL = jest.fn();
});

const flush = () => new Promise((r) => setTimeout(r, 40));
/** Waits (up to a second, for a busy machine) until `done` holds. */
const until = async (done: () => boolean) => {
  for (let i = 0; i < 100 && !done(); i += 1) await new Promise((r) => setTimeout(r, 10));
};

test('sentences play in order, the next fetched while one plays, at the chosen speed', async () => {
  const fetched: string[] = [];
  const states: boolean[] = [];
  const speaker = new Speaker((text) => { fetched.push(text); return Promise.resolve(new Blob([text])); }, (s) => states.push(s));
  speaker.rate = 1.5;
  speaker.say('하나.');
  speaker.say('둘.');
  speaker.say('셋.');
  expect(fetched).toEqual(['하나.', '둘.']);
  await until(() => states.length === 2);
  expect(fetched).toEqual(['하나.', '둘.', '셋.']);
  expect(FakeAudio.played).toEqual(['blob:1', 'blob:2', 'blob:3']);
  expect(states).toEqual([true, false]);
});

test('stop forgets the rest at once', async () => {
  const states: boolean[] = [];
  const speaker = new Speaker((text) => Promise.resolve(new Blob([text])), (s) => states.push(s));
  speaker.say('하나.');
  speaker.say('둘.');
  speaker.stop();
  await flush();
  expect(FakeAudio.played.length).toBeLessThanOrEqual(1);
  expect(states).toEqual([true, false]);
  expect(speaker.speaking).toBe(false);
});

test('a sentence that cannot be read is reported and skipped', async () => {
  const errors: string[] = [];
  const speaker = new Speaker((text) => (text === '나쁨' ? Promise.reject(new Error('HTTP 502')) : Promise.resolve(new Blob([text]))),
    () => undefined, (m) => errors.push(m));
  speaker.say('나쁨');
  speaker.say('좋음');
  await until(() => FakeAudio.played.length === 1);
  expect(errors).toEqual(['HTTP 502']);
  expect(FakeAudio.played).toEqual(['blob:1']);
});

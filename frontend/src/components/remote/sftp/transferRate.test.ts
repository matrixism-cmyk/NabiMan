import { formatDuration, startRate, updateRate, RateView } from './transferRate';

const KO = { h: '시간', m: '분', s: '초' };
const MB = 1024 * 1024;

/** Feeds readings every `stepMs` at a steady `perSecond`, returning the last view shown. */
function feed(perSecond: number, seconds: number, total: number, stepMs = 100) {
  let state = startRate(0);
  let shown: RateView | null = null;
  for (let now = stepMs; now <= seconds * 1000; now += stepMs) {
    const [next, view] = updateRate(state, (perSecond * now) / 1000, total, now);
    state = next;
    if (view) shown = view;
  }
  return { state, shown };
}

it('says nothing until it has seen a second of the transfer', () => {
  const { shown } = feed(10 * MB, 0.6, 1000 * MB);
  expect(shown).toEqual({ rate: null, eta: null });
});

it('settles on the steady rate and counts down what is left', () => {
  const { shown } = feed(10 * MB, 10, 1000 * MB);
  expect(shown!.rate! / MB).toBeCloseTo(10, 0);
  // 100 MB sent of 1000 MB at 10 MB/s: 90 seconds left.
  expect(shown!.eta).toBeGreaterThanOrEqual(89);
  expect(shown!.eta).toBeLessThanOrEqual(91);
});

it('changes what it shows at most twice a second', () => {
  let state = startRate(0);
  let views = 0;
  for (let now = 50; now <= 5000; now += 50) {
    const [next, view] = updateRate(state, now * 1000, 10 ** 9, now);
    state = next;
    if (view) views += 1;
  }
  expect(views).toBeLessThanOrEqual(10);
});

it('measures afresh when a retried piece starts from an earlier point', () => {
  const { state } = feed(10 * MB, 3, 1000 * MB);
  const [after, view] = updateRate(state, 5 * MB, 1000 * MB, 3100);
  expect(view).toBeNull();
  expect(after.loaded).toBe(5 * MB);
  expect(after.rate).toBe(state.rate);
});

it('writes time the way people say it', () => {
  expect(formatDuration(45, KO)).toBe('45초');
  expect(formatDuration(200, KO)).toBe('3분 20초');
  expect(formatDuration(180, KO)).toBe('3분');
  expect(formatDuration(7500, KO)).toBe('2시간 5분');
  expect(formatDuration(7200, KO)).toBe('2시간');
  expect(formatDuration(200, { h: 'h', m: 'm', s: 's' })).toBe('3m 20s');
});

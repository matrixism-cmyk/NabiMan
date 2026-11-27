import { createKeeper } from './consoleKeeper';
import { createSnapshots, SHOT_FRESH_MS, SHOT_MAX, SHOT_WIDTH } from './consoleSnapshots';

/** A console session as the keeper and the snapshots see one. */
function fakeSession(kind: 'rdp' | 'vnc', size = { width: 1920, height: 1080 }) {
  const canvas = Object.assign(document.createElement('canvas'), size);
  const session = {
    status: 'connected', closed: false, canvas,
    close() { session.closed = true; session.status = 'disconnected'; },
    alive: () => !session.closed && session.status !== 'disconnected',
    watch: () => () => undefined,
    ...(kind === 'rdp'
      ? { client: { getDisplay: () => ({ flatten: () => canvas }) } }
      : { screen: (() => { const box = document.createElement('div'); box.appendChild(canvas); return box; })() }),
  };
  return session;
}

let drawn: { w: number; h: number }[] = [];
let read: jest.SpyInstance;
let clock = 1000;

beforeEach(() => {
  drawn = []; clock = 1000;
  localStorage.clear(); sessionStorage.clear();
  jest.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function context(this: HTMLCanvasElement) {
    return { drawImage: () => { drawn.push({ w: this.width, h: this.height }); } } as unknown as CanvasRenderingContext2D;
  } as never);
  read = jest.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockImplementation(() => `data:image/jpeg;base64,shot${drawn.length}`);
});
afterEach(() => { jest.restoreAllMocks(); });

const setup = () => {
  const keeper = createKeeper(() => clock);
  return { keeper, shots: createSnapshots(keeper, () => clock) };
};

test('a connected console gets a small picture, held in memory only', () => {
  const { keeper, shots } = setup();
  const changed = jest.fn();
  shots.subscribe(changed);
  keeper.adopt('win', fakeSession('rdp'));
  keeper.adopt('desk', fakeSession('vnc', { width: 200, height: 100 }));
  shots.refresh(['win', 'desk', 'never-opened']);
  expect(shots.shot('win')).toEqual({ url: 'data:image/jpeg;base64,shot1', at: 1000 });
  expect(drawn[0]).toEqual({ w: SHOT_WIDTH, h: 180 }); // scaled down, same shape
  expect(drawn[1]).toEqual({ w: 200, h: 100 }); // never scaled up
  expect(read).toHaveBeenCalledWith('image/jpeg', expect.any(Number));
  expect(shots.shot('never-opened')).toBeUndefined();
  expect(shots.held('win')).toBe(true);
  expect(shots.fresh('win')).toBe(true);
  expect(changed).toHaveBeenCalledTimes(1);
  expect(localStorage.length + sessionStorage.length).toBe(0);
});

test('nothing is taken from a console that is not connected here, or still connecting', () => {
  const { keeper, shots } = setup();
  const session = fakeSession('rdp');
  session.status = 'connecting';
  keeper.adopt('win', session);
  shots.refresh(['win', 'other']);
  expect(shots.size()).toBe(0);
  expect(shots.held('win')).toBe(false);
  expect(read).not.toHaveBeenCalled();
});

test('a console going out of sight is pictured as it was left', () => {
  const { keeper, shots } = setup();
  keeper.adopt('win', fakeSession('rdp'));
  expect(shots.shot('win')).toBeUndefined();
  keeper.hide('win');
  expect(shots.shot('win')?.url).toContain('data:image/jpeg');
});

test('hung up on purpose: the picture goes', () => {
  const { keeper, shots } = setup();
  keeper.adopt('win', fakeSession('rdp'));
  shots.refresh(['win']);
  keeper.drop('win');
  shots.forget('win');
  expect(shots.shot('win')).toBeUndefined();
  expect(shots.held('win')).toBe(false);
});

test('a connection that ended some other way keeps the screen last seen', () => {
  const { keeper, shots } = setup();
  const session = fakeSession('vnc');
  keeper.adopt('desk', session);
  shots.refresh(['desk']);
  keeper.hide('desk');
  clock += 5 * 60000;
  keeper.drop('desk'); // the keeper let it go: out of sight too long, or its place was needed
  shots.refresh(['desk']);
  expect(shots.shot('desk')?.at).toBe(1000);
  expect(shots.held('desk')).toBe(false);
  expect(shots.fresh('desk')).toBe(false);
});

test('a picture that could not be renewed stops counting as the screen now', () => {
  const { keeper, shots } = setup();
  keeper.adopt('win', fakeSession('rdp'));
  shots.refresh(['win']);
  read.mockImplementation(() => { throw new DOMException('tainted', 'SecurityError'); });
  clock += SHOT_FRESH_MS + 1;
  expect(() => shots.refresh(['win'])).not.toThrow();
  expect(shots.shot('win')?.at).toBe(1000);
  expect(shots.fresh('win')).toBe(false);
});

test('a screen that cannot be read has no picture, and says nothing', () => {
  const { keeper, shots } = setup();
  const errors = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  read.mockImplementation(() => { throw new DOMException('tainted', 'SecurityError'); });
  keeper.adopt('win', fakeSession('rdp'));
  keeper.adopt('blank', fakeSession('vnc', { width: 0, height: 0 }));
  const noCanvas = fakeSession('vnc');
  noCanvas.canvas.remove();
  keeper.adopt('empty', noCanvas);
  expect(() => shots.refresh(['win', 'blank', 'empty'])).not.toThrow();
  read.mockImplementation(() => 'data:,'); // what a browser answers for a canvas it cannot encode
  shots.refresh(['win']);
  expect(shots.size()).toBe(0);
  expect(errors).not.toHaveBeenCalled();
});

test('consoles the account may no longer open, and signing out, leave no picture', () => {
  const { keeper, shots } = setup();
  ['a', 'b', 'c'].forEach((key) => keeper.adopt(key, fakeSession('rdp')));
  shots.refresh(['a', 'b', 'c']);
  shots.retain(['a', 'c']);
  expect(shots.shot('b')).toBeUndefined();
  expect(shots.size()).toBe(2);
  shots.clear();
  expect(shots.size()).toBe(0);
});

test('only so many pictures are remembered: the oldest go first', () => {
  const { shots } = setup();
  const sessions = new Map<string, ReturnType<typeof fakeSession>>();
  const many = createSnapshots({ get: (key) => sessions.get(key), hidden: () => [], subscribe: () => () => undefined }, () => clock);
  const keys = Array.from({ length: SHOT_MAX + 3 }, (_, i) => `vm${i}`);
  keys.forEach((key) => sessions.set(key, fakeSession('rdp')));
  many.refresh(keys);
  expect(many.size()).toBe(SHOT_MAX);
  expect(many.shot('vm0')).toBeUndefined();
  expect(many.shot(`vm${SHOT_MAX + 2}`)).toBeDefined();
  expect(shots.size()).toBe(0);
});

test('the shared store drops everything when the account signs out', () => {
  jest.isolateModules(() => {
    const { consoleKeeper } = require('./consoleKeeper');
    const { consoleSnapshots } = require('./consoleSnapshots');
    consoleKeeper.adopt('win', fakeSession('rdp'));
    consoleSnapshots.refresh(['win']);
    expect(consoleSnapshots.size()).toBe(1);
    window.dispatchEvent(new Event('nabiman_auth_changed'));
    expect(consoleSnapshots.size()).toBe(0);
  });
});

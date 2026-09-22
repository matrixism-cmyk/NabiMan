/**
 * The clipboard path into a terminal: each shortcut must paste exactly once,
 * and a file on the clipboard must arrive as a path the shell can open.
 */
import { attachClipboard } from './terminalChrome';
import { blobsFromClipboardItems, pathsForClipboardFiles } from './pasteFile';

jest.mock('./pasteFile', () => ({
  pathsForClipboardFiles: jest.fn(),
  blobsFromClipboardItems: jest.fn(),
}));

const SAVED_PATH = '/var/lib/nabiman/pastes/admin/paste-20260922-130000-ab12.png';

// jest.config.js resets mock implementations between tests, so they are set here.
beforeEach(() => {
  (pathsForClipboardFiles as jest.Mock).mockResolvedValue(SAVED_PATH);
  (blobsFromClipboardItems as jest.Mock).mockResolvedValue([]);
});

type KeyHandler = (ev: KeyboardEvent) => boolean;

/** Just enough of an xterm terminal for the clipboard wiring to attach to. */
function fakeTerminal() {
  let keyHandler: KeyHandler = () => true;
  return {
    term: {
      parser: { registerOscHandler: () => ({ dispose: () => {} }) },
      attachCustomKeyEventHandler: (h: KeyHandler) => { keyHandler = h; },
      hasSelection: () => false,
      getSelection: () => '',
    },
    press: (ev: KeyboardEvent) => keyHandler(ev),
  };
}

function setup() {
  const sent: string[] = [];
  const container = document.createElement('div');
  document.body.appendChild(container);
  const fake = fakeTerminal();
  const detach = attachClipboard(fake.term as never, container, (t) => sent.push(t));
  return { sent, container, fake, detach };
}

function pasteEvent(data: { text?: string; files?: File[] }): ClipboardEvent {
  const ev = new Event('paste', { bubbles: true, cancelable: true }) as ClipboardEvent;
  Object.defineProperty(ev, 'clipboardData', {
    value: {
      getData: () => data.text || '',
      files: data.files || [],
    },
  });
  return ev;
}

describe('terminal clipboard', () => {
  it('pastes text once for a paste event', () => {
    const { sent, container, detach } = setup();
    container.dispatchEvent(pasteEvent({ text: 'echo hi' }));
    expect(sent).toEqual(['echo hi']);
    detach();
  });

  // Ctrl+Shift+V is the browser's own paste-as-plain-text: it fires a paste
  // event by itself. Pasting from the key handler too made every Ctrl+Shift+V
  // arrive twice.
  it('does not paste twice on Ctrl+Shift+V', async () => {
    const { sent, container, fake, detach } = setup();
    const handled = fake.press(new KeyboardEvent('keydown', {
      key: 'V', ctrlKey: true, shiftKey: true, cancelable: true,
    }));
    expect(handled).toBe(false); // xterm must not send the keystroke on
    container.dispatchEvent(pasteEvent({ text: 'ls -al' }));
    await new Promise((r) => setTimeout(r, 500));
    expect(sent).toEqual(['ls -al']);
    detach();
  });

  // ...and where the browser keeps the shortcut to itself, the key handler is
  // the only one who can paste, so it still has to.
  it('reads the clipboard when Ctrl+Shift+V fires no paste event', async () => {
    const readText = jest.fn().mockResolvedValue('from-clipboard');
    Object.defineProperty(navigator, 'clipboard', {
      value: { readText, writeText: jest.fn() }, configurable: true,
    });
    const { sent, fake, detach } = setup();
    fake.press(new KeyboardEvent('keydown', {
      key: 'V', ctrlKey: true, shiftKey: true, cancelable: true,
    }));
    await new Promise((r) => setTimeout(r, 500));
    expect(sent).toEqual(['from-clipboard']);
    detach();
  });

  it('pastes the server path for a file on the clipboard', async () => {
    const { sent, container, detach } = setup();
    const file = new File([new Uint8Array([1, 2, 3])], 'shot.png', { type: 'image/png' });
    container.dispatchEvent(pasteEvent({ files: [file] }));
    await new Promise((r) => setTimeout(r, 0));
    expect(pathsForClipboardFiles).toHaveBeenCalledWith([file]);
    expect(sent).toEqual([SAVED_PATH]);
    detach();
  });
});

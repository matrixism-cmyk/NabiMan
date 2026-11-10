import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { TerminalSettingsProvider } from './settings';
import { TerminalWindowsProvider, useTerminalWindows } from './TerminalWindows';

// The window manager is what is under test; a real xterm pane needs a canvas.
jest.mock('./TerminalView', () => {
  const react = require('react');
  return {
    __esModule: true,
    default: react.forwardRef(function FakeTerminalView(props: { onSessionId?: (id: string) => void }) {
      // Set by a test that needs the window attached to a session.
      const sid = (globalThis as any).fakeSessionId;
      react.useEffect(() => { if (sid) props.onSessionId?.(sid); }, [sid]);
      return react.createElement('div', { 'data-testid': 'fake-terminal' });
    }),
    forgetTerminalStorage: jest.fn(),
    pruneTerminalStorage: jest.fn(),
    storedSessionId: () => '',
  };
});

// A plain function, not jest.fn(): CRA resets mock implementations between
// tests, which would make the settings provider await `undefined`.
jest.mock('../../shared/api/useApi', () => ({
  sessionSubject: () => null,
  apiRequest: (path: string) => Promise.resolve({
    success: true, message: '',
    data: path === '/api/terminal/sessions' ? [{ id: 'pty_named', title: '빌드 서버' }] : null,
  }),
}));

function Opener() {
  const { openTerminal } = useTerminalWindows();
  return (
    <button onClick={() => openTerminal({ kind: 'ssh', host: '10.0.0.1', port: 22, user: 'root' }, { title: 'web-1' })}>
      open
    </button>
  );
}

function renderDesktop() {
  return render(
    <I18nProvider>
      <TerminalSettingsProvider>
        <TerminalWindowsProvider>
          <Opener />
        </TerminalWindowsProvider>
      </TerminalSettingsProvider>
    </I18nProvider>,
  );
}

function openWindow() {
  fireEvent.click(screen.getByText('open'));
  return document.querySelector('.term-window') as HTMLElement;
}

beforeEach(() => {
  delete (globalThis as any).fakeSessionId;
  localStorage.clear();
  window.confirm = jest.fn(() => true);
});

describe('terminal windows', () => {
  it('shows a renamed session\'s name after the title in the taskbar', async () => {
    (globalThis as any).fakeSessionId = 'pty_named';
    renderDesktop();
    openWindow();
    await waitFor(() => expect(document.querySelector('.term-taskbar-item')).toHaveTextContent('web-1 - 빌드 서버'));
  });

  it('opens a window for a server', () => {
    renderDesktop();
    const win = openWindow();
    expect(win).toBeTruthy();
    expect(screen.getByText('web-1')).toBeInTheDocument();
    expect(screen.getByText('root@10.0.0.1:22')).toBeInTheDocument();
  });

  it('closes on ✕ even after the title bar took a pointer down (drag start)', () => {
    renderDesktop();
    const win = openWindow();
    const bar = win.querySelector('.term-window-bar') as HTMLElement;
    const close = win.querySelector('.term-window-close') as HTMLElement;
    // A press lands on the bar first; it must not swallow the button's click.
    fireEvent.pointerDown(close, { bubbles: true, button: 0 });
    fireEvent.pointerUp(close, { bubbles: true });
    fireEvent.click(close);
    expect(document.querySelector('.term-window')).toBeNull();
    expect(bar).not.toBeInTheDocument();
  });

  it('minimises and restores from the taskbar', () => {
    renderDesktop();
    openWindow();
    const minimise = screen.getByTitle('Minimise');
    fireEvent.pointerDown(minimise, { bubbles: true, button: 0 });
    fireEvent.click(minimise);
    expect(document.querySelector('.term-window-minimized')).toBeTruthy();
    // The pane stays mounted so its screen and connection survive.
    expect(screen.getByTestId('fake-terminal')).toBeInTheDocument();

    fireEvent.click(screen.getByTitle('root@10.0.0.1:22'));
    expect(document.querySelector('.term-window-minimized')).toBeNull();
  });

  it('maximises and restores the previous geometry', () => {
    renderDesktop();
    const win = openWindow();
    const maximise = screen.getByTitle('Maximise');
    const before = win.style.width;
    fireEvent.pointerDown(maximise, { bubbles: true, button: 0 });
    fireEvent.click(maximise);
    expect((document.querySelector('.term-window') as HTMLElement).style.width).toBe('100vw');
    fireEvent.click(maximise);
    expect((document.querySelector('.term-window') as HTMLElement).style.width).toBe(before);
  });

  it('ends a session only after confirmation', async () => {
    renderDesktop();
    openWindow();
    const end = screen.getByTitle('End session (also closes the remote shell)');
    (window.confirm as jest.Mock).mockReturnValueOnce(false);
    fireEvent.click(end);
    await waitFor(() => expect(window.confirm).toHaveBeenCalledTimes(1));
    expect(document.querySelector('.term-window')).toBeTruthy();
    fireEvent.click(end);
    await waitFor(() => expect(document.querySelector('.term-window')).toBeNull());
  });

  it('tiles several windows without dropping any', () => {
    renderDesktop();
    openWindow(); openWindow(); openWindow();
    expect(document.querySelectorAll('.term-window')).toHaveLength(3);
    const [tileBtn] = Array.from(document.querySelectorAll('.term-taskbar-btn')) as HTMLElement[];
    fireEvent.click(tileBtn);
    const boxes = Array.from(document.querySelectorAll('.term-window')) as HTMLElement[];
    expect(boxes).toHaveLength(3);
    // Tiled windows must not all sit on top of each other.
    expect(new Set(boxes.map((b) => `${b.style.left}|${b.style.top}`)).size).toBe(3);
  });

  it('never captures the pointer for a title-bar button (clicks would be swallowed)', () => {
    renderDesktop();
    const win = openWindow();
    const bar = win.querySelector('.term-window-bar') as HTMLElement;
    const capture = jest.fn();
    // jsdom has no pointer capture; stubbing it shows what a browser would do.
    (bar as unknown as { setPointerCapture: unknown }).setPointerCapture = capture;
    const close = win.querySelector('.term-window-close') as HTMLElement;

    fireEvent.pointerDown(close, { bubbles: true, button: 0, clientX: 10, clientY: 10 });
    expect(capture).not.toHaveBeenCalled();

    // Pressing the bar itself still starts a drag.
    fireEvent.pointerDown(bar, { bubbles: true, button: 0, clientX: 10, clientY: 10 });
    expect(capture).toHaveBeenCalled();
  });

  it('remembers open windows across a reload', () => {
    const first = renderDesktop();
    openWindow();
    first.unmount();
    renderDesktop();
    expect(screen.getByText('web-1')).toBeInTheDocument();
  });
});

import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { TerminalSettingsProvider } from './settings';
import { TerminalWindowsProvider, useTerminalWindows } from './TerminalWindows';

// The window manager is what is under test; a real xterm pane needs a canvas.
jest.mock('./TerminalView', () => {
  const react = require('react');
  return {
    __esModule: true,
    default: react.forwardRef(function FakeTerminalView() {
      return react.createElement('div', { 'data-testid': 'fake-terminal' });
    }),
    forgetTerminalStorage: jest.fn(),
    pruneTerminalStorage: jest.fn(),
    storedSessionId: () => '',
  };
});

// A plain function, not jest.fn(): CRA resets mock implementations between
// tests, which would make the settings provider await `undefined`.
jest.mock('../../hooks/useApi', () => ({
  apiRequest: () => Promise.resolve({ success: true, data: null, message: '' }),
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
  localStorage.clear();
  window.confirm = jest.fn(() => true);
});

describe('terminal windows', () => {
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
    const win = openWindow();
    const minimise = win.querySelectorAll('.term-window-actions button')[2] as HTMLElement;
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
    const maximise = win.querySelectorAll('.term-window-actions button')[3] as HTMLElement;
    const before = win.style.width;
    fireEvent.pointerDown(maximise, { bubbles: true, button: 0 });
    fireEvent.click(maximise);
    expect((document.querySelector('.term-window') as HTMLElement).style.width).toBe('100vw');
    fireEvent.click(maximise);
    expect((document.querySelector('.term-window') as HTMLElement).style.width).toBe(before);
  });

  it('ends a session only after confirmation', () => {
    renderDesktop();
    const win = openWindow();
    const end = win.querySelectorAll('.term-window-actions button')[4] as HTMLElement;
    (window.confirm as jest.Mock).mockReturnValueOnce(false);
    fireEvent.click(end);
    expect(document.querySelector('.term-window')).toBeTruthy();
    fireEvent.click(end);
    expect(document.querySelector('.term-window')).toBeNull();
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

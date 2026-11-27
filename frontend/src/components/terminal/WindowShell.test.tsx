import React, { useState } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import type { TermWindow, WindowPatch } from './windowGeometry';
import WindowShell from './WindowShell';

const base = { x: 100, y: 120, w: 700, h: 400, z: 11, minimized: false, maximized: false, title: 't', subtitle: 's' };
const KINDS: Record<string, TermWindow> = {
  terminal: { ...base, id: 'w', kind: 'terminal', target: { kind: 'local' } },
  console: { ...base, id: 'w', kind: 'console', console: { name: 'vm', label: 'vm', protocol: 'rdp', builtin: true } },
};

// jsdom's pointer events carry no coordinates: a mouse event under the pointer's name does.
const pointer = (type: 'Down' | 'Move' | 'Up', el: Element, at: { clientX: number; clientY: number }) => {
  act(() => { el.dispatchEvent(new MouseEvent(`pointer${type.toLowerCase()}`, { bubbles: true, cancelable: true, button: 0, ...at })); });
};

const focused = jest.fn();
function Desk({ start }: { start: TermWindow }) {
  const [win, setWin] = useState(start);
  return (
    <WindowShell win={win} onFocus={focused} onUpdate={(_id: string, patch: WindowPatch) => setWin((w) => ({ ...w, ...patch }))}
      lead={<span>title</span>} actions={<button>own</button>} closing={<button className="term-window-close">✕</button>}>
      <div data-testid="body" />
    </WindowShell>
  );
}
const show = (kind: string, more: Partial<TermWindow> = {}) =>
  render(<I18nProvider><Desk start={{ ...KINDS[kind], ...more } as TermWindow} /></I18nProvider>);
const frame = () => document.querySelector('.term-window') as HTMLElement;
const bar = () => document.querySelector('.term-window-bar') as HTMLElement;
const box = () => { const s = frame().style; return [s.left, s.top, s.width, s.height]; };

beforeEach(() => {
  localStorage.clear(); localStorage.setItem('nabiman_lang', 'ko');
  focused.mockClear();
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
  Object.defineProperty(window, 'innerHeight', { value: 1000, configurable: true });
});
afterEach(() => { delete (document as { elementFromPoint?: unknown }).elementFromPoint; });

describe.each(['terminal', 'console'])('the frame of a %s window', (kind) => {
  test('is dragged by its title bar and raised by the press', () => {
    show(kind);
    expect(frame().dataset.windowKind).toBe(kind);
    pointer('Down', bar(), { clientX: 300, clientY: 130 });
    expect(focused).toHaveBeenCalledWith('w');
    pointer('Move', bar(), { clientX: 350, clientY: 190 });
    expect(box()).toEqual(['150px', '180px', '700px', '400px']);
    pointer('Up', bar(), { clientX: 350, clientY: 190 });
    pointer('Move', bar(), { clientX: 900, clientY: 600 });
    expect(box()).toEqual(['150px', '180px', '700px', '400px']);
  });

  test('a press on a title-bar button starts no drag', () => {
    show(kind);
    pointer('Down', screen.getByText('own'), { clientX: 300, clientY: 130 });
    pointer('Move', bar(), { clientX: 400, clientY: 300 });
    expect(box()).toEqual(['100px', '120px', '700px', '400px']);
    expect(focused).not.toHaveBeenCalled();
  });

  test('is resized by its corner, never under the smallest size', () => {
    show(kind);
    const corner = document.querySelector('.term-resize-se') as HTMLElement;
    pointer('Down', corner, { clientX: 800, clientY: 520 });
    pointer('Move', corner, { clientX: 900, clientY: 560 });
    expect(box()).toEqual(['100px', '120px', '800px', '440px']);
    pointer('Move', corner, { clientX: 0, clientY: 0 });
    expect(box()).toEqual(['100px', '120px', '320px', '200px']);
  });

  test('maximises and comes back to its place; minimised it stays mounted', () => {
    show(kind);
    fireEvent.click(screen.getByTitle('최대화'));
    expect(box()).toEqual(['0px', '0px', '100vw', 'calc(100vh - 44px)']);
    fireEvent.click(screen.getByTitle('이전 크기'));
    expect(box()).toEqual(['100px', '120px', '700px', '400px']);
    fireEvent.doubleClick(bar());
    expect(frame().style.width).toBe('100vw');
    fireEvent.click(screen.getByTitle('최소화'));
    expect(frame()).toHaveClass('term-window-minimized');
    expect(screen.getByTestId('body')).toBeInTheDocument();
  });

  test('a maximised window comes loose when really dragged, at the size it had', () => {
    show(kind, { maximized: true, restore: { x: 100, y: 120, w: 700, h: 400 } });
    pointer('Down', bar(), { clientX: 960, clientY: 10 });
    pointer('Move', bar(), { clientX: 961, clientY: 12 });
    expect(frame().style.width).toBe('100vw'); // a mere click of the bar
    pointer('Move', bar(), { clientX: 960, clientY: 210 });
    expect(box()).toEqual(['610px', '200px', '700px', '400px']);
  });

  test('dragged to the top it shows the arrangements, and dropped on a zone takes that place', () => {
    show(kind);
    pointer('Down', bar(), { clientX: 300, clientY: 130 });
    pointer('Move', bar(), { clientX: 300, clientY: 5 });
    const zone = document.querySelector('[data-snap="1:1"]') as HTMLElement;
    expect(zone).toBeTruthy();
    (document as { elementFromPoint?: unknown }).elementFromPoint = () => zone;
    pointer('Move', bar(), { clientX: 310, clientY: 40 });
    pointer('Up', bar(), { clientX: 310, clientY: 40 });
    // The right half of the room above the bar.
    expect(box()).toEqual(['963px', '3px', '954px', '950px']);
    expect(document.querySelector('[data-snap]')).toBeNull();
    // Maximising and restoring from there returns to the snapped place.
    fireEvent.click(screen.getByTitle('최대화'));
    fireEvent.click(screen.getByTitle('이전 크기'));
    expect(box()).toEqual(['963px', '3px', '954px', '950px']);
  });
});

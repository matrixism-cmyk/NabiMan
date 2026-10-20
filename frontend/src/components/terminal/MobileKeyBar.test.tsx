import React, { createRef } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import MobileKeyBar from './MobileKeyBar';
import type { TerminalViewHandle } from './terminalTypes';

const handle = () => {
  const ref = createRef<TerminalViewHandle>() as { current: Partial<TerminalViewHandle> };
  ref.current = { pressKey: jest.fn(), armModifiers: jest.fn(), zoomBy: jest.fn(), focus: jest.fn(), insertText: jest.fn() };
  return ref;
};
const ui = (ref: ReturnType<typeof handle>) => render(
  <I18nProvider><MobileKeyBar handle={ref as React.RefObject<TerminalViewHandle>} /></I18nProvider>);

beforeEach(() => localStorage.setItem('nabiman_lang', 'ko'));

test('a key types on release, with Ctrl held for it, then Ctrl lets go', () => {
  const ref = handle();
  ui(ref);
  fireEvent.click(screen.getByRole('button', { name: 'Ctrl' }));
  expect(ref.current.armModifiers).toHaveBeenLastCalledWith({ ctrl: true, alt: false }, expect.any(Function));
  fireEvent.pointerDown(screen.getByRole('button', { name: '위' }));
  expect(ref.current.pressKey).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '위' }));
  expect(ref.current.pressKey).toHaveBeenCalledWith('ArrowUp', { ctrl: true, alt: false });
  expect(ref.current.armModifiers).toHaveBeenLastCalledWith({ ctrl: false, alt: false }, expect.any(Function));
  expect(screen.getByRole('button', { name: 'Ctrl' })).toHaveAttribute('aria-pressed', 'false');
});

test('symbols type as they are, and A+ zooms', () => {
  const ref = handle();
  ui(ref);
  fireEvent.click(screen.getByRole('button', { name: '|' }));
  expect(ref.current.pressKey).toHaveBeenCalledWith('|', { ctrl: false, alt: false });
  fireEvent.click(screen.getByRole('button', { name: '글자 크게' }));
  expect(ref.current.zoomBy).toHaveBeenCalledWith(1);
});

test('a held arrow repeats, and its release adds no extra press', () => {
  jest.useFakeTimers();
  const ref = handle();
  ui(ref);
  const down = screen.getByRole('button', { name: '아래' });
  fireEvent.pointerDown(down);
  jest.advanceTimersByTime(400 + 70 * 3);
  fireEvent.pointerLeave(down);
  fireEvent.click(down);
  expect((ref.current.pressKey as jest.Mock).mock.calls.length).toBe(4);
  jest.useRealTimers();
});

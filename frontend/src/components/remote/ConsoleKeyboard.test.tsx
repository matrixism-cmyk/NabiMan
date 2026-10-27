import React, { useRef } from 'react';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { useConsoleKeyboard } from './ConsoleKeyboard';

const sent: string[] = [];
function Console() {
  const host = useRef<HTMLDivElement>(null);
  const kbd = useConsoleKeyboard({ enabled: true, screen: host, send: (k, down) => { if (down) sent.push(k.toString(16)); } });
  return <div>{kbd.button}<div ref={host} data-testid="screen" tabIndex={0} />{kbd.panel}</div>;
}

beforeAll(() => Object.defineProperty(navigator, 'maxTouchPoints', { value: 5, configurable: true }));
beforeEach(() => { localStorage.setItem('nabiman_lang', 'ko'); sent.length = 0; });

test('a tap on the screen keeps the phone keyboard up, and brings it back once put away', () => {
  render(<I18nProvider><Console /></I18nProvider>);
  const toggle = screen.getByRole('button', { name: '화면 키보드' });
  fireEvent.click(toggle);
  const box = document.querySelector('textarea') as HTMLTextAreaElement;
  expect(document.activeElement).toBe(box);

  // The screen takes the focus (a tap beside the desktop): it comes straight back.
  act(() => screen.getByTestId('screen').focus());
  expect(document.activeElement).toBe(box);

  // The phone put its keyboard away; the mode stays on, and a tap raises it again.
  act(() => box.blur());
  expect(toggle).toHaveAttribute('aria-pressed', 'true');
  fireEvent.touchEnd(screen.getByTestId('screen'), { touches: [] });
  expect(document.activeElement).toBe(box);

  // ⌨ again turns it off; taps are then only taps.
  fireEvent.click(toggle);
  expect(toggle).toHaveAttribute('aria-pressed', 'false');
  fireEvent.touchEnd(screen.getByTestId('screen'), { touches: [] });
  expect(document.activeElement).not.toBe(box);
});

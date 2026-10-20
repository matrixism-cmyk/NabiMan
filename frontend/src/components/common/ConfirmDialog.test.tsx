import React from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import { ConfirmHost, confirmDialog } from './ConfirmDialog';
import Modal from './Modal';

beforeEach(() => localStorage.setItem('nabiman_lang', 'ko'));

const host = () => render(<I18nProvider><ConfirmHost /></I18nProvider>);

test('the answer comes back from the modal, not the browser', async () => {
  window.confirm = jest.fn(() => true);
  host();
  let answer: Promise<boolean> = Promise.resolve(false);
  act(() => { answer = confirmDialog({ message: '지울까요?', confirmLabel: '삭제', danger: true, items: ['a.txt', 'b/'] }); });
  const dialog = await screen.findByRole('alertdialog');
  expect(dialog).toHaveTextContent('지울까요?');
  expect(screen.getByText('b/')).toBeInTheDocument();
  // A destructive question starts on cancel.
  expect(screen.getByRole('button', { name: '취소' })).toHaveFocus();
  fireEvent.click(screen.getByRole('button', { name: '삭제' }));
  await expect(answer).resolves.toBe(true);
  expect(window.confirm).not.toHaveBeenCalled();
  expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
});

test('Escape and a click outside both say no; long lists are cut short', async () => {
  host();
  let first: Promise<boolean> = Promise.resolve(true);
  let second: Promise<boolean> = Promise.resolve(true);
  act(() => {
    first = confirmDialog({ message: 'one', items: Array.from({ length: 12 }, (_, i) => `f${i}`) });
    second = confirmDialog('two');
  });
  expect(await screen.findByText('외 4개')).toBeInTheDocument();
  fireEvent.keyDown(screen.getByRole('alertdialog'), { key: 'Escape' });
  await expect(first).resolves.toBe(false);
  // The next question waits its turn.
  expect(await screen.findByText('two')).toBeInTheDocument();
  fireEvent.mouseDown(document.querySelector('.common-modal-overlay') as Element);
  await expect(second).resolves.toBe(false);
});

test('without a host it falls back to the browser', async () => {
  window.confirm = jest.fn(() => false);
  await expect(confirmDialog('plain')).resolves.toBe(false);
  expect(window.confirm).toHaveBeenCalledWith('plain');
});

test('a modal keeps keys to itself and returns focus when it closes', async () => {
  const outer = jest.fn();
  const close = jest.fn();
  const { rerender } = render(<div onKeyDown={outer}><button>behind</button>
    <Modal title="t" onClose={close}><input aria-label="inside" /></Modal></div>);
  const inside = screen.getByRole('textbox', { name: 'inside' });
  expect(inside).toHaveFocus();
  fireEvent.keyDown(inside, { key: 'Delete' });
  expect(outer).not.toHaveBeenCalled();
  fireEvent.keyDown(inside, { key: 'Escape' });
  expect(close).toHaveBeenCalledTimes(1);
  // Focus that fell out of the box still lets Escape close the top modal.
  inside.blur();
  fireEvent.keyDown(document.body, { key: 'Escape' });
  expect(close).toHaveBeenCalledTimes(2);
  rerender(<div onKeyDown={outer}><button>behind</button></div>);
  await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
});

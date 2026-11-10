import React from 'react';
import { fireEvent, render, renderHook, act, screen } from '@testing-library/react';
import { I18nProvider } from '../../i18n';
import DismissibleMessage from './DismissibleMessage';
import { clickable, onActivateKey } from './activate';
import { useSortable } from '../../hooks/useSortable';
import ChangePasswordModal from '../../components/ChangePasswordModal';

beforeEach(() => localStorage.setItem('nabiman_lang', 'ko'));

it('Enter and Space do what a click does, but not inside a field', () => {
  const run = jest.fn();
  render(<div data-testid="card" {...clickable(run)}><input aria-label="inner" /></div>);
  const card = screen.getByTestId('card');
  expect(card).toHaveAttribute('role', 'button');
  expect(card).toHaveAttribute('tabindex', '0');
  fireEvent.keyDown(card, { key: 'Enter' });
  fireEvent.keyDown(card, { key: ' ' });
  fireEvent.keyDown(card, { key: 'a' });
  fireEvent.keyDown(screen.getByLabelText('inner'), { key: 'Enter' });
  expect(run).toHaveBeenCalledTimes(2);
  fireEvent.click(card);
  expect(run).toHaveBeenCalledTimes(3);
  expect(typeof onActivateKey(run)).toBe('function');
});

it('a dismissible message is a status with a close button', () => {
  const dismiss = jest.fn();
  render(<I18nProvider><DismissibleMessage onDismiss={dismiss}>저장했습니다</DismissibleMessage></I18nProvider>);
  expect(screen.getByRole('status')).toHaveTextContent('저장했습니다');
  fireEvent.click(screen.getByRole('button', { name: '닫기' }));
  expect(dismiss).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('status'));
  expect(dismiss).toHaveBeenCalledTimes(2);
});

it('sortable headings are reachable and announce their order', () => {
  const { result } = renderHook(() => useSortable([{ n: 2 }, { n: 1 }], 'n'));
  const props = result.current.headerProps('n');
  expect(props.tabIndex).toBe(0);
  expect(props['aria-sort']).toBe('ascending');
  expect(result.current.headerProps('other')['aria-sort']).toBe('none');
  act(() => props.onKeyDown({ key: 'Enter', target: 1, currentTarget: 1, preventDefault: () => {} } as unknown as React.KeyboardEvent));
  expect(result.current.headerProps('n')['aria-sort']).toBe('descending');
  expect(result.current.sorted.map((r) => r.n)).toEqual([2, 1]);
});

it('the password dialog is a real modal: named, focused, closed by Escape', () => {
  const close = jest.fn();
  render(<I18nProvider><ChangePasswordModal onClose={close} /></I18nProvider>);
  const dialog = screen.getByRole('dialog', { name: '비밀번호 변경' });
  expect(dialog).toHaveAttribute('aria-modal', 'true');
  expect(screen.getByLabelText('현재 비밀번호')).toHaveFocus();
  fireEvent.keyDown(screen.getByLabelText('현재 비밀번호'), { key: 'Escape' });
  expect(close).toHaveBeenCalled();
});

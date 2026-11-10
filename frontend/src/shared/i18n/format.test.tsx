import React from 'react';
import { act, render, screen } from '@testing-library/react';
import { I18nProvider, useT } from './context';
import { formatDate, formatDateTime, formatNumber, formatTime, localeOf, setFormatLang } from './format';

const at = Date.UTC(2026, 10, 11, 15, 4, 5);
const opts: Intl.DateTimeFormatOptions = { timeZone: 'UTC' };

afterEach(() => setFormatLang('ko'));

describe('format', () => {
  it('follows the chosen language, not the browser', () => {
    setFormatLang('en');
    expect(localeOf()).toBe('en-US');
    expect(formatTime(at, opts)).toBe('3:04:05 PM');
    expect(formatDateTime(at, opts)).not.toMatch(/오후|午後/);
    setFormatLang('ko');
    expect(formatTime(at, opts)).toMatch(/^오후 3:04:05$/);
    setFormatLang('ja');
    expect(formatTime(at, opts)).toBe('15:04:05');
    expect(formatDate(at, opts)).toBe('2026/11/11');
  });

  it('takes dates, ISO strings and milliseconds, and passes options through', () => {
    const want = formatDateTime(new Date(at), opts, 'en');
    expect(formatDateTime('2026-11-11T15:04:05Z', opts, 'en')).toBe(want);
    expect(formatDateTime(at, opts, 'en')).toBe(want);
    expect(formatDateTime(at, { ...opts, month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }, 'en'))
      .toBe('11/11, 15:04');
    expect(formatDate(at, opts, 'ko')).toBe('2026. 11. 11.');
  });

  it('gives an unreadable value back as it was', () => {
    expect(formatDateTime('not a date')).toBe('not a date');
    expect(formatTime('')).toBe('');
  });

  it('groups numbers', () => {
    expect(formatNumber(1234567, undefined, 'en')).toBe('1,234,567');
    expect(formatNumber(1234567, undefined, 'ja')).toBe('1,234,567');
  });
});

function Clock() {
  const { setLang } = useT();
  return <><span data-testid="when">{formatTime(at, opts)}</span><button onClick={() => setLang('en')}>en</button></>;
}

it('the provider keeps the format language in step with the chosen one', () => {
  localStorage.setItem('nabiman_lang', 'ko');
  render(<I18nProvider translations={{}}><Clock /></I18nProvider>);
  expect(screen.getByTestId('when')).toHaveTextContent('오후');
  act(() => { screen.getByRole('button').click(); });
  expect(screen.getByTestId('when')).toHaveTextContent('3:04:05 PM');
});

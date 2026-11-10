import React from 'react';
import { act, renderHook } from '@testing-library/react';
import { I18nProvider, useT } from './index';
import { I18nProvider as Frame, interpolate } from '../shared/i18n/context';

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <I18nProvider>{children}</I18nProvider>
);

describe('i18n t()', () => {
  it('falls back to the key itself when a translation is missing', () => {
    const { result } = renderHook(() => useT(), { wrapper });
    expect(result.current.t('this.key.does.not.exist')).toBe('this.key.does.not.exist');
  });

  it('resolves a known key to real copy', () => {
    const { result } = renderHook(() => useT(), { wrapper });
    const label = result.current.t('tab.mecNoc');
    expect(label).not.toBe('tab.mecNoc');
    expect(label.length).toBeGreaterThan(0);
  });

  it('interpolates {var} placeholders', () => {
    const { result } = renderHook(() => useT(), { wrapper });
    const text = result.current.t('noc.sig.podFailed', { n: 3 });
    expect(text).toContain('3');
    expect(text).not.toContain('{n}');
  });
});

describe('i18n engine', () => {
  it('fills in every occurrence of a placeholder, not only the first', () => {
    expect(interpolate('{n} of {n} ({who})', { n: 3, who: 'ops' })).toBe('3 of 3 (ops)');
    expect(interpolate('no vars {x}')).toBe('no vars {x}');
  });

  it('t() repeats a variable everywhere the translation uses it', () => {
    const words = { 'x.twice': { ko: '{name}님, {name}님의 세션', en: '{name}, the session of {name}', ja: '{name}さん、{name}さんのセッション' } };
    localStorage.setItem('nabiman_lang', 'en');
    const { result } = renderHook(() => useT(), {
      wrapper: ({ children }: { children: React.ReactNode }) => <Frame translations={words}>{children}</Frame>,
    });
    expect(result.current.t('x.twice', { name: 'kim' })).toBe('kim, the session of kim');
  });

  it('sets <html lang> to the language in use, and follows a change', () => {
    localStorage.setItem('nabiman_lang', 'ja');
    document.documentElement.lang = 'ko';
    const { result } = renderHook(() => useT(), { wrapper });
    expect(document.documentElement.lang).toBe('ja');
    act(() => result.current.setLang('en'));
    expect(document.documentElement.lang).toBe('en');
  });
});

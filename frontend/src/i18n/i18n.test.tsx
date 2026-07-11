import React from 'react';
import { renderHook } from '@testing-library/react';
import { I18nProvider, useT } from './index';

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

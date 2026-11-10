import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { I18nProvider } from '../../../i18n';
import GraphSettingsPanel from './GraphSettingsPanel';
import { DEFAULT_SETTINGS, FORCE_DEFAULTS } from './view/graphSettings';

const setup = (extra: object = {}) => {
  localStorage.setItem('nabiman_lang', 'ko');
  const onChange = jest.fn();
  const onSearch = jest.fn();
  const onSearchEnter = jest.fn();
  render(<I18nProvider><GraphSettingsPanel settings={{ ...DEFAULT_SETTINGS, sections: { filters: true, groups: true, display: true, forces: true } }} onChange={onChange}
    search="" onSearch={onSearch} matches={0} onSearchEnter={onSearchEnter} groups={[['local', '#5eead4'], ['spark', '#a78bfa']]} devices={3} tiered {...extra} /></I18nProvider>);
  return { onChange, onSearch, onSearchEnter };
};

it('switches the style, the toggles and the sliders', () => {
  const { onChange } = setup();
  expect(screen.getByRole('button', { name: '심플' })).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(screen.getByRole('button', { name: '화려하게' }));
  expect(onChange).toHaveBeenLastCalledWith({ style: 'rich' });
  fireEvent.click(screen.getByRole('switch', { name: '종료된 세션' }));
  expect(onChange).toHaveBeenLastCalledWith({ showEnded: false });
  fireEvent.change(screen.getByLabelText('이름이 보이는 거리'), { target: { value: '0.8' } });
  expect(onChange).toHaveBeenLastCalledWith({ labelThreshold: 0.8 });
  fireEvent.click(screen.getByRole('button', { name: '기본값으로' }));
  expect(onChange).toHaveBeenLastCalledWith({ ...FORCE_DEFAULTS });
  // The legend lives here now, and the tiers' note shows while they hold the heights.
  expect(screen.getByText('이 서버')).toBeInTheDocument();
  expect(screen.getByText(/높이가 고정되어/)).toBeInTheDocument();
});

it('folds a section and searches', () => {
  const { onChange, onSearch, onSearchEnter } = setup({ search: 'spark', matches: 2 });
  fireEvent.click(screen.getByRole('button', { name: /힘/ }));
  expect(onChange).toHaveBeenLastCalledWith({ sections: { filters: true, groups: true, display: true, forces: false } });
  const box = screen.getByRole('searchbox', { name: '노드 찾기' });
  fireEvent.change(box, { target: { value: 'x' } });
  expect(onSearch).toHaveBeenLastCalledWith('x');
  fireEvent.keyDown(box, { key: 'Enter' });
  expect(onSearchEnter).toHaveBeenCalled();
  expect(screen.getByText(/2개 찾음/)).toBeInTheDocument();
});

import fs from 'fs';
import path from 'path';
import React from 'react';
import { render } from '@testing-library/react';
import StatusBadge from '../../components/mec/common/StatusBadge';

/**
 * Colours in components come from the theme (shared/styles/tokens.css), so the
 * light theme and the dark ones both read well. Hex values stay only where a
 * colour is data, not chrome: chart series, session-link hues, the terminal's
 * own palettes and the dark ground behind a remote screen.
 */
const SRC = path.join(__dirname, '..', '..');
const ALLOWED = new Set([
  'components/ChartsPanel.tsx', // chart series
  'components/DisksPanel.tsx', // chart series
  'components/mec/MecGpuPanel.tsx', // one hue per GPU model
  'components/mec/common/DonutChart.tsx', // categorical palette
  'components/remote/SessionLinks.tsx', // one hue per link
  'components/remote/SerialMonitorPopout.tsx', // xterm theme needs a hex value
  'components/remote/VncConsoleView.tsx', // noVNC canvas ground
  'components/terminal/terminalChrome.ts', // terminal colour schemes
]);
const SKIP_DIRS = ['vendor', path.join('components', 'remote', 'graph')]; // the graph themes itself
const HEX = /['"`\s(]#(?:[0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/;

function walk(dir: string, out: string[] = []): string[] {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const rel = path.relative(SRC, full);
    if (entry.isDirectory()) { if (!SKIP_DIRS.includes(rel)) walk(full, out); continue; }
    if (/\.tsx?$/.test(entry.name) && !/\.test\.tsx?$/.test(entry.name)) out.push(rel.split(path.sep).join('/'));
  }
  return out;
}

it('components take their colours from theme tokens', () => {
  const offenders: string[] = [];
  for (const rel of walk(path.join(SRC, 'components')).concat(walk(path.join(SRC, 'shared')))) {
    if (ALLOWED.has(rel) || rel === 'shared/theme/presets.ts') continue;
    fs.readFileSync(path.join(SRC, rel), 'utf8').split('\n').forEach((line, i) => {
      if (!/^\s*(\/\/|\*)/.test(line) && HEX.test(line)) offenders.push(`${rel}:${i + 1}: ${line.trim()}`);
    });
  }
  expect(offenders).toEqual([]);
});

it('every token the components use is defined', () => {
  const css = fs.readFileSync(path.join(SRC, 'shared', 'styles', 'tokens.css'), 'utf8')
    + fs.readFileSync(path.join(SRC, 'shared', 'styles', 'base.css'), 'utf8')
    + fs.readFileSync(path.join(SRC, 'shared', 'styles', 'design.css'), 'utf8');
  for (const name of ['info', 'running', 'text-muted', 'track', 'on-accent', 'screen-bg', 'screen-fg',
    ...['success', 'warning', 'danger', 'info', 'running'].flatMap((tone) => [`${tone}-soft`, `${tone}-strong`])]) {
    expect(css).toMatch(new RegExp(`--${name}:`));
  }
  // The light theme has its own tints, not the dark ones'.
  expect(css).toMatch(/:root\[data-theme-mode="light"\]\s*\{[^}]*--danger-strong:/);
});

it('a status badge is drawn with theme tokens', () => {
  const { container } = render(<StatusBadge tone="failed">failed</StatusBadge>);
  const badge = container.firstChild as HTMLElement;
  expect(badge.style.background).toBe('var(--danger-soft)');
  expect(badge.style.color).toBe('var(--danger-strong)');
});

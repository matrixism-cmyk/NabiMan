import { DEFAULT_SETTINGS, LEGACY_NETWORK_KEY, LEGACY_ROTATE_KEY, loadSettings, sanitize, saveSettings, SETTINGS_KEY } from './graphSettings';

beforeEach(() => localStorage.clear());

it('starts simple, with the defaults', () => {
  const s = loadSettings();
  expect(s).toEqual(DEFAULT_SETTINGS);
  expect(s.style).toBe('simple');
  expect(s.panel).toBe(false);
});

it('remembers what was set, and falls back on bad JSON', () => {
  saveSettings({ ...DEFAULT_SETTINGS, style: 'rich', labelThreshold: 0.8, panel: true });
  expect(JSON.parse(localStorage.getItem(SETTINGS_KEY)!).style).toBe('rich');
  expect(loadSettings()).toMatchObject({ style: 'rich', labelThreshold: 0.8, panel: true });
  localStorage.setItem(SETTINGS_KEY, '{not json');
  expect(loadSettings()).toEqual(DEFAULT_SETTINGS);
});

it('takes the old rotate and network toggles over when nothing is saved yet', () => {
  localStorage.setItem(LEGACY_ROTATE_KEY, '0');
  localStorage.setItem(LEGACY_NETWORK_KEY, '0');
  expect(loadSettings()).toMatchObject({ autoRotate: false, showDevices: false });
});

it('mends unknown, wrong-typed and out-of-range values', () => {
  const s = sanitize({ style: 'neon', nodeSize: 99, repel: -4, arrows: 'yes', linkDistance: NaN, replaySpeed: 7, sections: { forces: true, filters: 3 } });
  expect(s.style).toBe('simple');
  expect(s.nodeSize).toBe(2.5);
  expect(s.repel).toBe(0.1);
  expect(s.arrows).toBe(true);
  expect(s.linkDistance).toBe(DEFAULT_SETTINGS.linkDistance);
  expect(s.replaySpeed).toBe(DEFAULT_SETTINGS.replaySpeed);
  expect(s.sections).toEqual({ ...DEFAULT_SETTINGS.sections, forces: true });
  expect(sanitize('nonsense')).toEqual(DEFAULT_SETTINGS);
});

it('survives storage that refuses', () => {
  const broken = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('full'); } };
  expect(loadSettings(broken)).toEqual(DEFAULT_SETTINGS);
  expect(() => saveSettings(DEFAULT_SETTINGS, broken)).not.toThrow();
  expect(loadSettings(null)).toEqual(DEFAULT_SETTINGS);
});

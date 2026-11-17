import { DEFAULT_SPEED, readSpeed, rememberSpeed, VNC_LEVELS } from './consoleSpeed';

describe('console speed', () => {
  beforeEach(() => { localStorage.clear(); });

  it('is balanced until chosen, and remembered per console', () => {
    expect(DEFAULT_SPEED).toBe('balanced');
    expect(readSpeed('win11')).toBe('balanced');
    rememberSpeed('win11', 'fast');
    rememberSpeed('win2022', 'sharp');
    expect(readSpeed('win11')).toBe('fast');
    expect(readSpeed('win2022')).toBe('sharp');
    expect(readSpeed('other')).toBe('balanced');
  });

  it('falls back to balanced on anything it does not know', () => {
    localStorage.setItem('nabiman_console_speed', JSON.stringify({ a: 'ultra', b: 3 }));
    expect(readSpeed('a')).toBe('balanced');
    expect(readSpeed('b')).toBe('balanced');
    localStorage.setItem('nabiman_console_speed', 'not json');
    expect(readSpeed('a')).toBe('balanced');
    localStorage.setItem('nabiman_console_speed', '[]');
    expect(readSpeed('0')).toBe('balanced');
  });

  it('survives a browser that refuses storage', () => {
    const set = jest.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('denied'); });
    const get = jest.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('denied'); });
    expect(() => rememberSpeed('win11', 'fast')).not.toThrow();
    expect(readSpeed('win11')).toBe('balanced');
    set.mockRestore(); get.mockRestore();
  });

  it('gives VNC a sharper picture as the speed goes down', () => {
    expect(VNC_LEVELS.fast.quality).toBeLessThan(VNC_LEVELS.balanced.quality);
    expect(VNC_LEVELS.balanced.quality).toBeLessThan(VNC_LEVELS.sharp.quality);
    // noVNC's own defaults.
    expect(VNC_LEVELS.balanced).toEqual({ quality: 6, compression: 2 });
  });
});

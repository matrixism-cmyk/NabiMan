/**
 * What a terminal pane keeps in the browser, and — more importantly — what it
 * must not throw away. Clearing the wrong keys costs the operator the zoom,
 * the restored screen and the session id, which sends the pane off to start a
 * fresh shell instead of re-attaching to the one still running.
 */
import {
  bufKey, fontKeyForTest, pruneSessionZooms, pruneTerminalStorage, readFontSize, sessionZoomKey,
  sidKey, writeFontSize,
} from './terminalStorage';

beforeEach(() => localStorage.clear());

describe('pruneTerminalStorage', () => {
  it('clears panes of floating windows that are gone', () => {
    localStorage.setItem(bufKey('tw_gone'), 'screen');
    localStorage.setItem(sidKey('tw_gone'), 'pty_1');
    localStorage.setItem(fontKeyForTest('tw_gone'), '20');
    localStorage.setItem(bufKey('tw_open'), 'screen');

    pruneTerminalStorage(['tw_open']);

    expect(localStorage.getItem(bufKey('tw_gone'))).toBeNull();
    expect(localStorage.getItem(sidKey('tw_gone'))).toBeNull();
    expect(localStorage.getItem(fontKeyForTest('tw_gone'))).toBeNull();
    expect(localStorage.getItem(bufKey('tw_open'))).toBe('screen');
  });

  // The window list knows nothing about these panes, and used to wipe them on
  // every page load.
  it('leaves workspace, docked and share panes alone', () => {
    localStorage.setItem(sidKey('ws_local'), 'pty_local');
    localStorage.setItem(fontKeyForTest('ws_local'), '18');
    localStorage.setItem(bufKey('dock_main'), 'screen');
    localStorage.setItem(fontKeyForTest('share_abc'), '16');
    localStorage.setItem(fontKeyForTest(sessionZoomKey('pty_local')), '18');

    pruneTerminalStorage([]);

    expect(localStorage.getItem(sidKey('ws_local'))).toBe('pty_local');
    expect(localStorage.getItem(fontKeyForTest('ws_local'))).toBe('18');
    expect(localStorage.getItem(bufKey('dock_main'))).toBe('screen');
    expect(localStorage.getItem(fontKeyForTest('share_abc'))).toBe('16');
    expect(localStorage.getItem(fontKeyForTest(sessionZoomKey('pty_local')))).toBe('18');
  });
});

describe('zoom per session', () => {
  it('keeps a size for each session', () => {
    writeFontSize(sessionZoomKey('pty_a'), 20);
    writeFontSize(sessionZoomKey('pty_b'), 11);

    expect(readFontSize(sessionZoomKey('pty_a'))).toBe(20);
    expect(readFontSize(sessionZoomKey('pty_b'))).toBe(11);
    expect(readFontSize(sessionZoomKey('pty_c'))).toBeNull();
  });

  it('forgets the size of sessions that have ended', () => {
    writeFontSize(sessionZoomKey('pty_alive'), 20);
    writeFontSize(sessionZoomKey('pty_dead'), 11);
    writeFontSize('ws_local', 16);

    pruneSessionZooms(['pty_alive']);

    expect(readFontSize(sessionZoomKey('pty_alive'))).toBe(20);
    expect(readFontSize(sessionZoomKey('pty_dead'))).toBeNull();
    expect(readFontSize('ws_local')).toBe(16);
  });
});

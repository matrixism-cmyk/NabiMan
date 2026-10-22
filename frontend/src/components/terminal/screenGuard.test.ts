/**
 * A terminal's stored screen belongs to the account that printed it: gone at
 * sign-out and when someone else signs in at the same browser.
 */
import { bufKey, ENDED_SID, guardScreens, sidKey, storedSessionId, writeStored } from './terminalStorage';

const token = (sub: string) => `x.${btoa(JSON.stringify({ sub })).replace(/=+$/, '')}.y`;
const signIn = (sub: string) => {
  sessionStorage.setItem('nabiman_token', token(sub));
  window.dispatchEvent(new Event('nabiman_auth_changed'));
};
const signOut = () => {
  sessionStorage.removeItem('nabiman_token');
  window.dispatchEvent(new Event('nabiman_auth_changed'));
};

beforeEach(() => { localStorage.clear(); sessionStorage.clear(); });

it('keeps an account\'s screens across its own sign-ins, and drops them for anyone else', () => {
  signIn('alice');
  writeStored(bufKey('ws_local'), 'alice typed a password');
  localStorage.setItem(sidKey('ws_local'), 'pty_1');
  signIn('alice');
  expect(localStorage.getItem(bufKey('ws_local'))).toBe('alice typed a password');
  signIn('bob');
  expect(localStorage.getItem(bufKey('ws_local'))).toBeNull();
});

it('never forgets which session a pane was on, or that it ended: that would open new empty shells', () => {
  localStorage.setItem(sidKey('ws_local'), 'pty_live');
  localStorage.setItem(sidKey('tw_old'), ENDED_SID);
  writeStored(bufKey('ws_local'), 'screen');
  // The first page load of this code: nobody recorded yet.
  sessionStorage.setItem('nabiman_token', token('alice'));
  guardScreens();
  signIn('bob');
  signOut();
  expect(storedSessionId('ws_local')).toBe('pty_live');
  expect(localStorage.getItem(sidKey('tw_old'))).toBe(ENDED_SID);
});

it('drops them at sign-out, and a pane closing afterwards cannot write them back', () => {
  signIn('alice');
  writeStored(bufKey('tw_1'), 'secret');
  signOut();
  expect(localStorage.getItem(bufKey('tw_1'))).toBeNull();
  writeStored(bufKey('tw_1'), 'secret, saved on unmount');
  expect(localStorage.getItem(bufKey('tw_1'))).toBeNull();
});

it('leaves a page that has not signed in yet alone, and a share visitor keeps their own', () => {
  signIn('alice');
  writeStored(bufKey('ws_local'), 'screen');
  sessionStorage.clear();
  guardScreens();
  expect(localStorage.getItem(bufKey('ws_local'))).toBe('screen');
  writeStored(bufKey('share_abc'), 'visitor screen');
  expect(localStorage.getItem(bufKey('share_abc'))).toBe('visitor screen');
});

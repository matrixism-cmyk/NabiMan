import { ALT_R, CONTROL_R, HANGUL, HANJA, remoteKey } from './rdpKeys';

describe('remoteKey', () => {
  it('sends 한/영 and 한자 as a tap of right Alt / right Ctrl', () => {
    expect(remoteKey(HANGUL)).toEqual({ keysym: ALT_R, tap: true });
    expect(remoteKey(HANJA)).toEqual({ keysym: CONTROL_R, tap: true });
  });
  it('passes every other key through as press/release', () => {
    expect(remoteKey(0x61)).toEqual({ keysym: 0x61, tap: false });
    expect(remoteKey(ALT_R)).toEqual({ keysym: ALT_R, tap: false });
  });
});

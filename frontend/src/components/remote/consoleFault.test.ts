import { describeFault, DISPLACED, reconnectDelay, RECONNECT_MAX } from './consoleFault';

const t = (key: string) => key;

describe('describeFault', () => {
  it('does not retry when another login took the Windows session (SESSION_CONFLICT)', () => {
    expect(DISPLACED).toBe(0x0209);
    expect(describeFault(t, { code: 0x0209, message: 'RDP server closed/refused connection: Disconnected by other connection.' }))
      .toEqual({ message: 'remote.console.err.displaced', retry: false });
    expect(describeFault(t, { code: 0x0209 })).toEqual({ message: 'remote.console.err.displaced', retry: false });
  });

  it('knows the displaced message even without its code', () => {
    // The words "refused connection" alone would read as an unreachable host, which retries.
    expect(describeFault(t, { message: 'RDP server closed/refused connection: Disconnected by other connection.' }).retry).toBe(false);
  });

  it('still retries an ordinary drop', () => {
    expect(describeFault(t)).toEqual({ message: 'remote.console.lost', retry: true });
    expect(describeFault(t, { code: 0x0208 })).toEqual({ message: 'remote.console.err.unreachable', retry: true });
    expect(describeFault(t, { code: 0x0202 })).toEqual({ message: 'remote.console.err.timeout', retry: true });
    // An idle or time-limit close by the server is not a displacement.
    expect(describeFault(t, { code: 0x020a }).retry).toBe(true);
  });

  it('never retries a refused login or a missing console', () => {
    expect(describeFault(t, { code: 0x0301 })).toEqual({ message: 'remote.console.err.auth', retry: false });
    expect(describeFault(t, { message: 'securityfailure' }).retry).toBe(false);
    expect(describeFault(t, { code: 0x0204 })).toEqual({ message: 'remote.console.err.notfound', retry: false });
  });

  it('backs off between attempts, up to a cap', () => {
    expect([1, 2, 3, 4].map(reconnectDelay)).toEqual([1500, 3000, 6000, 8000]);
    expect(RECONNECT_MAX).toBe(4);
  });
});

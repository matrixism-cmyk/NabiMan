/** A console connection failure, from guacd (RDP, with a Guacamole status code)
 *  or noVNC (RFB, message only). */
export interface ConsoleFault { code?: number; message?: string }

type TFunc = (key: string, params?: Record<string, string | number>) => string;

/**
 * Explain a console disconnect in Korean and decide whether to auto-retry.
 * Credential/permission failures never retry — hammering a locked or disabled
 * account (e.g. a Windows box after bad logins) only locks it longer; the user
 * is told to fix the account instead. Transient faults (host unreachable,
 * timeout, dropped session) retry with backoff.
 */
export function describeFault(t: TFunc, fault?: ConsoleFault): { message: string; retry: boolean } {
  const code = fault?.code;
  const msg = (fault?.message || '').toLowerCase();
  const authish = /denied|locked|disabled|credential|logon|authentic|unauthor|password|permission|security/.test(msg);
  // 0x0301 CLIENT_UNAUTHORIZED, 0x0303 CLIENT_FORBIDDEN.
  if (code === 0x0301 || code === 0x0303 || authish) return { message: t('remote.console.err.auth'), retry: false };
  // 0x0204 RESOURCE_NOT_FOUND, 0x0207 UPSTREAM_NOT_FOUND.
  if (code === 0x0204 || code === 0x0207) return { message: t('remote.console.err.notfound'), retry: false };
  // 0x0208 UPSTREAM_UNAVAILABLE, 0x0203 UPSTREAM_ERROR.
  if (code === 0x0208 || code === 0x0203 || /refused|unreachable|no route|connect/.test(msg)) {
    return { message: t('remote.console.err.unreachable'), retry: true };
  }
  // 0x0202 UPSTREAM_TIMEOUT, 0x020A SESSION_TIMEOUT.
  if (code === 0x0202 || code === 0x020a || /timeout|timed out/.test(msg)) {
    return { message: t('remote.console.err.timeout'), retry: true };
  }
  return { message: fault?.message || t('remote.console.lost'), retry: true };
}

/** Backoff before the n-th reconnect attempt (1-based): 1.5s, 3s, 6s, capped 8s. */
export const RECONNECT_MAX = 4;
export const reconnectDelay = (n: number): number => Math.min(1500 * 2 ** (n - 1), 8000);

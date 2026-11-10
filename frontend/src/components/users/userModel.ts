import type { MemberUser } from '../MembershipEditor';
import type { TFn } from '../../i18n';
import { formatDateTime, formatDate } from '../../shared/i18n/format';

export type Role = 'admin' | 'remote' | 'viewer' | 'operator';
export type Status = MemberUser['status'];

/** Translation keys of each level's name. */
export const ROLE_LABEL: Record<string, string> = {
  admin: 'users.role.admin',
  remote: 'users.role.remote',
  viewer: 'users.role.viewer',
  operator: 'users.role.operator',
};

/** One line on what each level gets, for the choice and the detail (translation keys). */
export const ROLE_HINT: Record<string, string> = {
  admin: 'users.roleHint.admin',
  remote: 'users.roleHint.remote',
  viewer: 'users.roleHint.viewer',
  operator: 'users.roleHint.operator',
};

/** A level's name, or the raw level when it is one this screen does not know. */
export const roleName = (t: TFn, role: string) => (ROLE_LABEL[role] ? t(ROLE_LABEL[role]) : role);

export const STATUS_LABEL: Record<Status, string> = {
  active: 'users.status.active',
  pending: 'users.status.pending',
  rejected: 'users.status.rejected',
  suspended: 'users.status.suspended',
};

export const STATUS_TONE: Record<Status, string> = {
  active: 'is-ok',
  pending: 'is-warn',
  rejected: 'is-muted',
  suspended: 'is-bad',
};

/** "2026-10-22T07:43:12Z", "2026-10-22 07:43:12" or a unix second, as local time. */
export function when(value?: string | number | null): string {
  if (value === undefined || value === null || value === '') return '—';
  const date = typeof value === 'number'
    ? new Date(value * 1000)
    : new Date(/^\d{4}-\d\d-\d\d \d/.test(value) ? `${value.replace(' ', 'T')}Z` : value);
  return Number.isNaN(date.getTime()) ? String(value) : formatDateTime(date);
}

/** How long ago, roughly, for a list. */
export function ago(t: TFn, value?: string | null): string {
  if (!value) return t('users.ago.never');
  const date = new Date(/^\d{4}-\d\d-\d\d \d/.test(value) ? `${value.replace(' ', 'T')}Z` : value);
  const secs = (Date.now() - date.getTime()) / 1000;
  if (Number.isNaN(secs)) return value;
  if (secs < 60) return t('users.ago.now');
  if (secs < 3600) return t('users.ago.minutes', { n: Math.floor(secs / 60) });
  if (secs < 86400) return t('users.ago.hours', { n: Math.floor(secs / 3600) });
  if (secs < 86400 * 30) return t('users.ago.days', { n: Math.floor(secs / 86400) });
  return formatDate(date);
}

/** Whether the account's time is over (the server agrees: it no longer signs in). */
export const expired = (u: MemberUser) => Boolean(u.access?.expires_at && Date.parse(u.access.expires_at) <= Date.now());

/** Short notes on the restrictions an account has, for the list. */
export function restrictions(t: TFn, u: MemberUser): string[] {
  const out: string[] = [];
  const a = u.access;
  if (a?.expires_at) out.push(expired(u) ? t('users.limit.expired') : `~${formatDate(a.expires_at)}`);
  if (a?.allowed_networks?.length) out.push(t('users.limit.ip'));
  if (a?.allowed_hours) out.push(t('users.limit.hours'));
  if (a?.require_mfa && !u.totp_enabled) out.push(t('users.limit.mfaNeeded'));
  return out;
}

export function matches(u: MemberUser, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || u.username.toLowerCase().includes(q);
}

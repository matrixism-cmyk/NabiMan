import type { MemberUser } from '../MembershipEditor';

export type Role = 'admin' | 'remote' | 'viewer' | 'operator';
export type Status = MemberUser['status'];

export const ROLE_LABEL: Record<string, string> = {
  admin: '최고관리자',
  remote: '원격 전용 회원',
  viewer: '일반 회원',
  operator: '기존 운영자',
};

/** One line on what each level gets, for the choice and the detail. */
export const ROLE_HINT: Record<string, string> = {
  admin: '서비스 전체를 관리합니다. 이용 기간·접속 제한을 둘 수 없습니다.',
  remote: '원격 메뉴만 씁니다. 본인이 등록한 서버와 관리자가 지정한 서버로 접속합니다.',
  viewer: '서버·tenant 소속별로 받은 권한만큼 씁니다.',
  operator: '예전 운영자 권한입니다. 새로 지정할 수는 없습니다.',
};

export const STATUS_LABEL: Record<Status, string> = {
  active: '승인',
  pending: '승인 대기',
  rejected: '반려',
  suspended: '이용 정지',
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
  return Number.isNaN(date.getTime()) ? String(value) : date.toLocaleString();
}

/** How long ago, roughly, for a list. */
export function ago(value?: string | null): string {
  if (!value) return '없음';
  const date = new Date(/^\d{4}-\d\d-\d\d \d/.test(value) ? `${value.replace(' ', 'T')}Z` : value);
  const secs = (Date.now() - date.getTime()) / 1000;
  if (Number.isNaN(secs)) return value;
  if (secs < 60) return '방금';
  if (secs < 3600) return `${Math.floor(secs / 60)}분 전`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}시간 전`;
  if (secs < 86400 * 30) return `${Math.floor(secs / 86400)}일 전`;
  return date.toLocaleDateString();
}

/** Whether the account's time is over (the server agrees: it no longer signs in). */
export const expired = (u: MemberUser) => Boolean(u.access?.expires_at && Date.parse(u.access.expires_at) <= Date.now());

/** Short notes on the restrictions an account has, for the list. */
export function restrictions(u: MemberUser): string[] {
  const out: string[] = [];
  const a = u.access;
  if (a?.expires_at) out.push(expired(u) ? '기간 만료' : `~${new Date(a.expires_at).toLocaleDateString()}`);
  if (a?.allowed_networks?.length) out.push('IP 제한');
  if (a?.allowed_hours) out.push('시간 제한');
  if (a?.require_mfa && !u.totp_enabled) out.push('2FA 등록 필요');
  return out;
}

export function matches(u: MemberUser, query: string): boolean {
  const q = query.trim().toLowerCase();
  return !q || u.username.toLowerCase().includes(q);
}

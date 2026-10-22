import React from 'react';
import './membership-management.css';
import { Membership } from './MembershipEditor';
export function accessActive(m: Membership) {
  return m.enabled !== false && (!m.expires_at || Date.parse(m.expires_at) > Date.now());
}
function localDate(iso?: string | null) {
  if (!iso) return '';
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  return new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
}
/** `remote`: a server assigned to a remote-only member, who never runs one-off commands. */
export default function MemberAccessFields({ value, onChange, remote = false }: { value: Membership; onChange: (m: Membership) => void; remote?: boolean }) {
  return <div className="member-access-fields">
    <label className="membership-check"><input type="checkbox" checked={value.enabled !== false} onChange={e => onChange({ ...value, enabled: e.target.checked })} /> {remote ? '사용 허용' : '소속 접근 허용'}</label>
    {value.kind === 'server' && !remote && <label className="membership-check"><input type="checkbox" checked={value.role !== 'viewer' && value.allow_exec !== false}
      disabled={value.role === 'viewer'} onChange={e => onChange({ ...value, allow_exec: e.target.checked })} /> 명령 실행 허용</label>}
    <label>접근 만료일 (현지 시간)<input type="datetime-local" value={localDate(value.expires_at)}
      onChange={e => onChange({ ...value, expires_at: e.target.value ? new Date(e.target.value).toISOString() : null })} /></label>
    <span>{value.expires_at ? '만료 후 자동으로 접근 차단' : '만료 제한 없음'}</span>
  </div>;
}

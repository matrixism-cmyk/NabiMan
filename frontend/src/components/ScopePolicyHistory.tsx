import React from 'react';
import { useApi } from '../hooks/useApi';
interface Change { revision: string; at: string; actor: string; actor_id: string; reason: string; fields: { field: string; before: unknown; after: unknown }[] }
const names: Record<string, string> = {contact_email: '담당자 이메일', member_notice: '회원 공지', member_limit: '회원 수 제한', allow_commands: '명령 실행 허용', require_mfa: '2단계 인증 필수'};
function value(input: unknown): string {
  if (input === null || input === '') return '없음';
  if (typeof input === 'boolean') return input ? '사용' : '사용 안 함';
  return String(input);
}
export default function ScopePolicyHistory({ endpoint }: { endpoint: string }) {
  const { data, loading, error, refetch } = useApi<Change[]>(`${endpoint}/history`);
  return <section aria-label="최근 정책 변경 이력">
    <h3>최근 정책 변경 이력</h3>
    <p>최근 100회 저장을 보관합니다. 이 기능 도입 이전 변경과 별도 감사 보관소의 기록은 포함하지 않습니다.</p>
    <button className="btn btn-secondary" onClick={refetch}>이력 새로고침</button>
    {loading && <p>이력을 불러오는 중…</p>}
    {error && <p role="alert">{error}</p>}
    {data?.length === 0 && <p>기록된 정책 변경이 없습니다.</p>}
    {data?.map(change => <details key={change.revision}>
      <summary>{new Date(change.at).toLocaleString()} · {change.actor} · {change.reason}</summary>
      <p>변경자: {change.actor} ({change.actor_id})</p>
      {change.fields.length === 0 ? <p>설정값 변경 없이 저장했습니다.</p> : <table className="data-table"><thead><tr><th>항목</th><th>변경 전</th><th>변경 후</th></tr></thead>
        <tbody>{change.fields.map(field => <tr key={field.field}><td>{names[field.field] || field.field}</td><td style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{value(field.before)}</td><td style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{value(field.after)}</td></tr>)}</tbody></table>}
    </details>)}
  </section>;
}

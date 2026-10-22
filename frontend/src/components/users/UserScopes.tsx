import React, { useState } from 'react';
import MembershipEditor, { Membership, MemberUser, ScopeOption } from '../MembershipEditor';
import TenantAccessInspection from '../TenantAccessInspection';

interface Props {
  user: MemberUser;
  options: ScopeOption[];
  busy: boolean;
  review: (status: MemberUser['status'], memberships: Membership[]) => Promise<boolean>;
}

/**
 * Where the account belongs: servers and tenants with a role in each, or for
 * a remote-only member, the administrators' servers they may use.
 */
export default function UserScopes({ user, options, busy, review }: Props) {
  const pending = user.status === 'pending';
  const remote = user.role === 'remote';
  const [draft, setDraft] = useState<Membership[]>(pending ? user.requested_memberships : user.memberships);
  const [inspecting, setInspecting] = useState<string | null>(null);
  const changed = JSON.stringify(draft) !== JSON.stringify(pending ? null : user.memberships);

  return (
    <div className="users-sections">
      <section className="users-card">
        <h4>{remote ? '관리자 지정 서버' : '서버·tenant 소속'}</h4>
        <p className="text-secondary">
          {remote
            ? '여기서 지정한 서버가 회원의 원격 목록에 "관리자 지정"으로 나옵니다. 터미널·파일·상태 확인만 할 수 있고, 접속은 서버에 설정된 방식(SSH 키·저장된 비밀번호)으로 이루어집니다.'
            : '소속마다 조회자·운영자·소속 관리자 중 하나를 정합니다. 사용 허용과 만료일로 접근을 잠시 막거나 기한을 둘 수 있습니다.'}
        </p>
        {pending && user.requested_memberships.length > 0 && (
          <p className="text-secondary">가입하며 신청한 소속이 미리 채워져 있습니다.</p>
        )}
        <MembershipEditor options={options} value={draft} onChange={setDraft} remote={remote} />
        <div className="btn-group">
          {pending
            ? <button type="button" className="btn btn-primary btn-sm" disabled={busy} onClick={() => review('active', draft)}>승인하고 저장</button>
            : <button type="button" className="btn btn-primary btn-sm" disabled={busy || !changed} onClick={() => review(user.status, draft)}>
              {remote ? '지정 서버 저장' : '소속 저장'}
            </button>}
          {!pending && changed && (
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setDraft(user.memberships)}>되돌리기</button>
          )}
        </div>
        <p className="text-secondary">저장하면 그 회원의 로그인이 모두 끝나고, 다시 로그인하면 바뀐 소속으로 열립니다.</p>
      </section>

      {user.memberships.some(m => m.kind === 'tenant') && (
        <section className="users-card">
          <h4>tenant 실제 접근</h4>
          <ul className="users-lines">
            {user.memberships.filter(m => m.kind === 'tenant').map(m => (
              <li key={m.scope_id}>
                <strong>{options.find(o => o.kind === 'tenant' && o.id === m.scope_id)?.label || m.scope_id}</strong>
                <button type="button" className="btn btn-sm btn-secondary" onClick={() => setInspecting(m.scope_id)}>실제 접근 확인</button>
              </li>
            ))}
          </ul>
          {inspecting && <TenantAccessInspection key={inspecting} tenantId={inspecting} userId={user.id}
            onClose={() => setInspecting(null)} />}
        </section>
      )}
    </div>
  );
}

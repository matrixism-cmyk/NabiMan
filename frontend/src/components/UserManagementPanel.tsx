import React from 'react';
import { useApi } from '../shared/api/useApi';
import type { MemberUser } from './MembershipEditor';
import UserAdmin from './users/UserAdmin';
import ScopeMembersPanel from './users/ScopeMembersPanel';

/**
 * 회원 관리: the super administrator manages every account; a scope manager
 * the members of the servers and tenants they manage.
 */
export default function UserManagementPanel({ initialScope = '' }: { initialScope?: string }) {
  const { data: me } = useApi<MemberUser>('/api/users/me');
  if (!me) return <div className="panel loading">회원 목록을 불러오는 중…</div>;
  return me.role === 'admin'
    ? <UserAdmin me={me} initialScope={initialScope} />
    : <ScopeMembersPanel me={me} initialScope={initialScope} />;
}

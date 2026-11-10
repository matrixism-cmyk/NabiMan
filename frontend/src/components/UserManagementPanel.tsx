import React from 'react';
import { useApi } from '../shared/api/useApi';
import type { MemberUser } from './MembershipEditor';
import UserAdmin from './users/UserAdmin';
import ScopeMembersPanel from './users/ScopeMembersPanel';
import { useT } from '../i18n';

/**
 * 회원 관리: the super administrator manages every account; a scope manager
 * the members of the servers and tenants they manage.
 */
export default function UserManagementPanel({ initialScope = '' }: { initialScope?: string }) {
  const { data: me } = useApi<MemberUser>('/api/users/me');
  const { t } = useT();
  if (!me) return <div className="panel loading">{t('users.loading')}</div>;
  return me.role === 'admin'
    ? <UserAdmin me={me} initialScope={initialScope} />
    : <ScopeMembersPanel me={me} initialScope={initialScope} />;
}

import React, { useState } from 'react';
import { apiRequest, useApi } from '../hooks/useApi';
import { useMecList } from '../hooks/mec/useMecApi';
import { Tenant } from '../types/mec';
import { MemberUser, ScopeOption } from './MembershipEditor';
import { accessActive } from './MemberAccessFields';
import ScopePolicyHistory from './ScopePolicyHistory';
import UserManagementPanel from './UserManagementPanel';
import { confirmDialog } from './common/ConfirmDialog';

interface Settings { revision: string; contact_email: string; member_notice: string; member_limit: number | null; allow_commands: boolean; require_mfa: boolean }
function ScopePolicy({ scope, canManage, tenant }: { scope: ScopeOption; canManage: boolean; tenant?: Tenant }) {
  const endpoint = `/api/scope-settings/${scope.kind}/${scope.id}`;
  const { data, loading, error, refetch } = useApi<Settings>(endpoint);
  const [draft, setDraft] = useState<Settings | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [reason, setReason] = useState('');
  const [conflict, setConflict] = useState(false);
  const [historyVersion, setHistoryVersion] = useState(0);
  const value = draft || data;
  if (loading) return <p>운영 설정을 불러오는 중…</p>;
  if (error || !value) return <p role="alert">{error || '운영 설정을 읽을 수 없습니다.'}</p>;
  const update = (change: Partial<Settings>) => setDraft({ ...value, ...change });
  return <div className="inline-form">
    <h3>{scope.label} · 운영 설정</h3>
    <form className="scope-policy-form" onSubmit={async e => {
      e.preventDefault(); setBusy(true); setMessage('');
      try {
        const { revision, ...settings } = value;
        const res = await apiRequest<string>(endpoint, { method: 'PUT', body: JSON.stringify({ ...settings, expected_revision: revision, change_reason: reason }) });
        setMessage(res.data || res.message);
        if (res.success) { await refetch(); setDraft(null); setReason(''); setConflict(false); setHistoryVersion(v => v + 1); }
        else if (res.code === 'scope_policy_conflict') setConflict(true);
      } catch { setMessage('설정을 저장하지 못했습니다.'); }
      finally { setBusy(false); }
    }}>
      <fieldset disabled={!canManage || busy} style={{ border: 0, padding: 0 }}>
        <div className="form-row"><label>담당자 이메일<input type="email" maxLength={254} value={value.contact_email} onChange={e => update({ contact_email: e.target.value })} /></label>
          <label>회원 수 제한<input type="number" min={1} max={10000} placeholder="제한 없음" value={value.member_limit ?? ''} onChange={e => update({ member_limit: e.target.value ? Number(e.target.value) : null })} /></label></div>
        <p>승인된 소속 회원 수에 적용됩니다. 접근을 중지한 회원도 포함되며 소속 해제 후 자리가 반환됩니다.</p>
        <label>소속 회원에게 표시할 공지<textarea rows={3} style={{ width: '100%', boxSizing: 'border-box' }} value={value.member_notice} onChange={e => update({ member_notice: e.target.value })} /></label>
        {scope.kind === 'server' && <p><label className="membership-check"><input type="checkbox" checked={value.allow_commands} onChange={e => update({ allow_commands: e.target.checked })} /> 소속 회원의 서버 명령 실행 허용</label><br />개별 회원의 운영 권한과 명령 실행 허용이 함께 필요합니다. 최고관리자의 복구 접근은 유지됩니다.</p>}
        <p><label className="membership-check"><input type="checkbox" checked={value.require_mfa || false} onChange={e => update({ require_mfa: e.target.checked })} /> 소속 회원의 2단계 인증 필수</label><br />
          활성 소속 중 하나라도 필수이면 해당 회원은 인증 앱 등록과 재로그인을 마칠 때까지 계정 보안 설정만 이용할 수 있습니다. 기존 로그인에도 적용됩니다. 최고관리자의 복구 접근은 유지됩니다.</p>
        <p>소속 관리자는 먼저 본인의 2단계 인증을 등록하고 다시 로그인해야 이 정책을 켤 수 있습니다. 외부 SSO·LDAP 계정의 인증 보장은 아직 연동되지 않아 필수 정책 아래에서는 접근이 차단됩니다.</p>
        {canManage && <><label>변경 사유<input type="text" minLength={3} maxLength={500} required value={reason} onChange={e => setReason(e.target.value)} style={{ display: 'block', width: '100%', textAlign: 'left' }} /></label>
          <button className="btn btn-primary" disabled={!draft || busy || conflict}>운영 설정 저장</button></>}
      </fieldset>
    </form>
    {message && <p role="status">{message}</p>}
    {conflict && <div role="alert"><p>작성한 내용은 아직 저장되지 않았습니다. 필요한 내용을 복사한 뒤 최신 설정을 불러와 다시 검토하세요.</p>
      <button className="btn btn-secondary" disabled={busy} onClick={async () => {
        if (!(await confirmDialog({ message: '작성한 변경 내용을 취소하고 최신 설정을 불러올까요?', danger: true }))) return;
        setBusy(true); await refetch(); setDraft(null); setConflict(false); setMessage(''); setBusy(false);
      }}>작성 내용 취소하고 최신 설정 불러오기</button></div>}
    {canManage && <ScopePolicyHistory key={`${endpoint}:${historyVersion}`} endpoint={endpoint} />}
    {tenant && <div>
      <h3>tenant 자원 할당량</h3>
      <table className="data-table"><thead><tr><th>CPU 요청 / 제한</th><th>메모리 요청 / 제한</th><th>GPU</th><th>namespace</th></tr></thead>
        <tbody><tr><td>{tenant.quota.cpu_requests} / {tenant.quota.cpu_limits}</td><td>{tenant.quota.memory_requests} / {tenant.quota.memory_limits}</td><td>{tenant.quota.gpu}</td><td>{tenant.namespace}</td></tr></tbody></table>
      <p>스토리지 {tenant.quota.storage} · 최대 Pod {tenant.quota.pods} · PVC {tenant.quota.pvcs} · LoadBalancer {tenant.quota.lb_services}</p>
      <p>현재 tenant 회원은 자원을 조회할 수 있습니다. 배포·재시작·삭제에 대한 tenant 운영 권한은 별도 확장이 필요합니다.</p>
      <p>자원 할당량·namespace·인프라 접속 정보는 최고관리자의 <a href="#mec/mecTenants">MEC tenant 관리</a>에서 확인합니다. 서비스 회원 권한과 Kubernetes 계정은 별도로 관리됩니다.</p>
    </div>}
  </div>;
}
export default function ScopeSettingsPanel() {
  const { data: me } = useApi<MemberUser>('/api/users/me');
  const { data: servers, error: serverError } = useApi<{ id: string; name: string }[]>('/api/remote-servers');
  const { data: tenants, error: tenantError } = useMecList<Tenant>('/api/mec/v1/tenants');
  const [selected, setSelected] = useState('');
  const [members, setMembers] = useState(false);
  const options: ScopeOption[] = [
    ...(tenants || []).map(t => ({ kind: 'tenant' as const, id: t.id, label: `tenant · ${t.display_name}` })),
    ...(servers || []).map(s => ({ kind: 'server' as const, id: s.id, label: `서버 · ${s.name}` })),
  ];
  const scope = options.find(s => `${s.kind}:${s.id}` === selected) || options[0];
  const canManage = me?.role === 'admin' || !!me?.memberships.some(m => scope && m.kind === scope.kind && m.scope_id === scope.id && m.role === 'admin' && accessActive(m));
  return <div className="panel scope-settings-panel">
    <h2>tenant·서버 운영 설정</h2>
    <p>소속을 선택해 운영 정책과 자원 정보를 확인하고, 해당 소속 회원의 접근 권한을 관리합니다.</p>
    {(serverError || tenantError) && <p role="alert">{serverError || tenantError}</p>}
    <label>관리할 소속 <select className="select-input" value={scope ? `${scope.kind}:${scope.id}` : ''} onChange={e => { setSelected(e.target.value); setMembers(false); }}>
      {options.map(s => <option key={`${s.kind}:${s.id}`} value={`${s.kind}:${s.id}`}>{s.label}</option>)}
    </select></label>
    {scope ? <>
      <div className="btn-group"><button className="btn btn-secondary" onClick={() => setMembers(false)}>운영 설정·자원</button>
        {canManage && <button className="btn btn-primary" onClick={() => setMembers(true)}>이 소속 회원 관리</button>}</div>
      {members && canManage ? <UserManagementPanel key={selected} initialScope={`${scope.kind}:${scope.id}`} />
        : <ScopePolicy key={`${scope.kind}:${scope.id}`} scope={scope} canManage={canManage} tenant={tenants?.find(t => scope.kind === 'tenant' && t.id === scope.id)} />}
    </> : <p>배정된 소속이 없습니다. 최고관리자는 원격 서버 또는 MEC tenant를 등록한 뒤 회원을 배정할 수 있습니다.</p>}
  </div>;
}

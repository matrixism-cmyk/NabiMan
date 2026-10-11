import React, { useEffect, useState } from 'react';
import { fetchWithRefresh } from '../hooks/useApi';
import { MemberUser } from './MembershipEditor';
import './membership-management.css';
interface Link { remote_user_id: string; remote_username: string; verified_at: string; linked_by: string; reason: string }
interface Snapshot { realm: string; link: Link | null }
export default function ExternalIdentityEditor({ user, onClose }: { user: MemberUser; onClose: () => void }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [remoteId, setRemoteId] = useState('');
  const [reason, setReason] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const endpoint = `/api/users/${encodeURIComponent(user.id)}/external-identities/rancher`;
  useEffect(() => {
    const controller = new AbortController();
    setSnapshot(null); setError(''); setRemoteId(''); setReason('');
    (async () => {
      try {
        const response = await fetchWithRefresh(endpoint, { signal: controller.signal });
        const body = await response.json();
        if (!response.ok || body.error || !body.data?.realm) throw new Error();
        if (!controller.signal.aborted) setSnapshot(body.data);
      } catch { if (!controller.signal.aborted) setError('연결 정보를 불러올 수 없습니다. Rancher 설정과 회원 승인 상태를 확인하세요.'); }
    })();
    return () => controller.abort();
  }, [endpoint]);
  return <section className="inline-form external-identity-editor" aria-label="Rancher 계정 연결">
    <h3>{user.username} · Rancher 계정 연결</h3>
    <p>실제 Rancher 계정 ID를 확인한 뒤 연결하세요. 이 연결만으로 프로젝트 접근 권한을 부여하거나 회수하지는 않습니다.</p>
    {error && <p role="alert">{error}</p>}
    {!snapshot && !error && <p>연결 정보를 불러오는 중…</p>}
    {snapshot && <>
      <p>대상 Rancher: <code>{snapshot.realm}</code></p>
      {snapshot.link ? <dl className="external-identity-details">
        <dt>연결된 계정</dt><dd>{snapshot.link.remote_username} · {snapshot.link.remote_user_id}</dd>
        <dt>확인 시각 / 관리자</dt><dd>{new Date(snapshot.link.verified_at).toLocaleString('ko-KR')} · {snapshot.link.linked_by}</dd>
        <dt>연결 사유</dt><dd>{snapshot.link.reason}</dd>
      </dl> : <form onSubmit={async e => {
        e.preventDefault(); setBusy(true); setError('');
        try {
          const response = await fetchWithRefresh(endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ realm: snapshot.realm, remote_user_id: remoteId, reason }) });
          const body = await response.json();
          if (!response.ok || body.error || !body.data?.remote_user_id) throw new Error();
          setSnapshot({ ...snapshot, link: body.data });
        } catch { setError('연결하지 못했습니다. 활성 Rancher 계정인지, 이미 연결된 계정인지 확인하고 다시 조회하세요.'); }
        finally { setBusy(false); }
      }}>
        <label>Rancher 계정 ID<input required maxLength={256} value={remoteId} onChange={e => setRemoteId(e.target.value)} /></label>
        <label>계정 확인 및 연결 사유<input required minLength={8} maxLength={1000} value={reason} onChange={e => setReason(e.target.value)} /></label>
        <button className="btn btn-primary" disabled={busy}>확인하고 연결</button>
      </form>}
    </>}
    <button className="btn btn-secondary" disabled={busy} onClick={onClose}>닫기</button>
  </section>;
}

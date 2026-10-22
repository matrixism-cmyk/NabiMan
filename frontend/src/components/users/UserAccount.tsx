import React, { useState } from 'react';
import { confirmDialog } from '../../shared/ui/ConfirmDialog';
import type { MemberUser } from '../MembershipEditor';
import ExternalIdentityEditor from '../ExternalIdentityEditor';
import { ROLE_HINT, ROLE_LABEL } from './userModel';

interface Props {
  user: MemberUser;
  self: boolean;
  busy: boolean;
  update: (body: object) => Promise<boolean>;
}

/** The account itself: its level, a new password, and its link to a Rancher account. */
export default function UserAccount({ user, self, busy, update }: Props) {
  const [role, setRole] = useState(user.role);
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [problem, setProblem] = useState('');
  const [linking, setLinking] = useState(false);
  const roles = ['remote', 'viewer', 'admin', ...(user.role === 'operator' ? ['operator'] : [])];

  const saveRole = async () => {
    if (role === user.role) return;
    if (role === 'admin' && !(await confirmDialog({ message: `${user.username} 에게 서비스 전체 최고관리자 권한을 줄까요?`, danger: true }))) return;
    if (user.role === 'admin' && !(await confirmDialog({ message: `${user.username} 의 최고관리자 권한을 거둘까요?`, danger: true }))) return;
    update({ role });
  };
  const savePassword = async () => {
    if (password.length < 8) { setProblem('비밀번호는 8자 이상으로 정하세요'); return; }
    if (password !== confirm) { setProblem('비밀번호 확인이 일치하지 않습니다'); return; }
    setProblem('');
    if (await update({ password })) { setPassword(''); setConfirm(''); }
  };

  return (
    <div className="users-sections">
      <section className="users-card">
        <h4>등급</h4>
        <div className="users-role-list" role="radiogroup" aria-label={`${user.username} 서비스 권한`}>
          {roles.map(r => (
            <label key={r} className={`users-role-choice ${role === r ? 'is-on' : ''}`}>
              <input type="radio" name={`role-${user.id}`} value={r} checked={role === r} disabled={self} onChange={() => setRole(r)} />
              <strong>{ROLE_LABEL[r]}</strong>
              <small>{ROLE_HINT[r]}</small>
            </label>
          ))}
        </div>
        {self ? <p className="text-secondary">자기 자신의 등급은 바꿀 수 없습니다.</p> : (
          <div className="btn-group">
            <button type="button" className="btn btn-primary btn-sm" disabled={busy || role === user.role} onClick={saveRole}>등급 저장</button>
          </div>
        )}
        <p className="text-secondary">등급을 바꾸면 그 회원의 로그인이 모두 끝나고, 다시 로그인하면 새 등급으로 열립니다.</p>
      </section>

      <section className="users-card">
        <h4>비밀번호 재설정</h4>
        {problem && <p role="alert" className="message">{problem}</p>}
        <div className="users-row">
          <label>새 비밀번호
            <input className="filter-input" type="password" autoComplete="new-password" value={password} onChange={e => setPassword(e.target.value)} />
          </label>
          <label>비밀번호 확인
            <input className="filter-input" type="password" autoComplete="new-password" value={confirm} onChange={e => setConfirm(e.target.value)} />
          </label>
        </div>
        <div className="btn-group">
          <button type="button" className="btn btn-secondary btn-sm" disabled={busy || !password} onClick={savePassword}>비밀번호 바꾸기</button>
        </div>
        <p className="text-secondary">바꾸면 그 회원의 로그인이 모두 끝납니다. 새 비밀번호는 따로 안전하게 전해 주세요.</p>
      </section>

      {user.status === 'active' && (
        <section className="users-card">
          <h4>외부 계정</h4>
          <p className="text-secondary">MEC(Rancher) 프로젝트 권한을 이 회원의 Rancher 계정과 연결합니다.</p>
          <div className="btn-group">
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setLinking(true)}>Rancher 계정 연결</button>
          </div>
          {linking && <ExternalIdentityEditor key={user.id} user={user} onClose={() => setLinking(false)} />}
        </section>
      )}
    </div>
  );
}

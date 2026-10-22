import React, { useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { apiPost } from '../../shared/api/useApi';
import { ROLE_HINT, ROLE_LABEL } from './userModel';

/** Making an account directly, approved from the start. */
export default function NewUserForm({ onClose, onCreated }: { onClose: () => void; onCreated: (username: string) => void }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [role, setRole] = useState('remote');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!/^[A-Za-z0-9_-]{2,32}$/.test(username)) { setMessage('아이디는 영문·숫자·밑줄·하이픈 2~32자로 입력하세요'); return; }
    if (password.length < 8) { setMessage('비밀번호는 8자 이상으로 입력하세요'); return; }
    if (password !== confirm) { setMessage('비밀번호 확인이 일치하지 않습니다'); return; }
    setBusy(true);
    try {
      const res = await apiPost<string>('/api/users', { username, password, role });
      if (res.success) onCreated(username);
      else setMessage(res.message || '회원을 만들 수 없습니다');
    } catch {
      setMessage('요청을 처리할 수 없습니다. 다시 시도하세요.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal onClose={onClose} title="회원 추가" className="users-new">
      <form className="users-form" onSubmit={submit}>
        {message && <p role="alert" className="message">{message}</p>}
        <label>아이디
          <input className="filter-input" value={username} onChange={e => setUsername(e.target.value)} autoComplete="off" required />
        </label>
        <label>비밀번호 (8자 이상)
          <input className="filter-input" type="password" value={password} onChange={e => setPassword(e.target.value)} autoComplete="new-password" required />
        </label>
        <label>비밀번호 확인
          <input className="filter-input" type="password" value={confirm} onChange={e => setConfirm(e.target.value)} autoComplete="new-password" required />
        </label>
        <fieldset className="users-roles">
          <legend>등급</legend>
          {['remote', 'viewer', 'admin'].map(r => (
            <label key={r} className={`users-role-choice ${role === r ? 'is-on' : ''}`}>
              <input type="radio" name="new-role" value={r} checked={role === r} onChange={() => setRole(r)} />
              <strong>{ROLE_LABEL[r]}</strong>
              <small>{ROLE_HINT[r]}</small>
            </label>
          ))}
        </fieldset>
        <p className="text-secondary">만든 회원은 바로 승인된 상태입니다. 원격 권한·지정 서버·접속 제한은 만든 뒤 상세에서 정합니다.</p>
        <div className="btn-group">
          <button type="submit" className="btn btn-primary" disabled={busy}>{busy ? '만드는 중…' : '회원 만들기'}</button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>취소</button>
        </div>
      </form>
    </Modal>
  );
}

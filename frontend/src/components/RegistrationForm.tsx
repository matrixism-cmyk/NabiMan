import React, { useState } from 'react';
import './registration-form.css';
import { useAuthCooldown } from '../hooks/useAuthCooldown';

export default function RegistrationForm({ onBack }: { onBack: () => void }) {
  const cooldown = useAuthCooldown();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [message, setMessage] = useState('');
  const [submitted, setSubmitted] = useState(false);
  const [busy, setBusy] = useState(false);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (cooldown.seconds || busy) return;
    if (password !== confirm) { setMessage('비밀번호 확인이 일치하지 않습니다.'); return; }
    setBusy(true); setMessage('');
    try {
      const res = await fetch('/api/auth/register', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      });
      if (cooldown.accept(res)) return;
      const json = await res.json();
      setMessage(json.message || json.data || '가입 신청을 처리하지 못했습니다.');
      if (json.success) { setSubmitted(true); setPassword(''); setConfirm(''); setMessage(json.data); }
    } catch { setMessage('서버에 연결할 수 없습니다. 잠시 후 다시 시도하세요.'); }
    finally { setBusy(false); }
  };
  return <div className="login-screen"><form className="login-form registration-form" onSubmit={submit}>
    <h1>회원가입 신청</h1>
    <p>최고관리자가 가입을 승인하고 서버·tenant 소속을 지정하면 서비스를 이용할 수 있습니다.</p>
    {message && <div role="status" className="message">{message}</div>}
    {cooldown.seconds > 0 && <p role="status">요청이 많습니다. {cooldown.seconds}초 후 다시 신청하세요.</p>}
    {!submitted && <>
      <label>아이디<input aria-label="가입 아이디" autoComplete="username" required minLength={2} maxLength={32}
        value={username} onChange={e => setUsername(e.target.value)} pattern="[A-Za-z0-9_-]+" /></label>
      <label>비밀번호 (12자 이상)<input aria-label="가입 비밀번호" type="password" autoComplete="new-password" required minLength={12}
        value={password} onChange={e => setPassword(e.target.value)} /></label>
      <label>비밀번호 확인<input aria-label="비밀번호 확인" type="password" autoComplete="new-password" required
        value={confirm} onChange={e => setConfirm(e.target.value)} /></label>
      <button className="btn btn-primary" disabled={busy || cooldown.seconds > 0}>{busy ? '신청 중…' : '가입 신청'}</button>
    </>}
    <button type="button" className="btn btn-secondary" onClick={onBack}>로그인으로 돌아가기</button>
  </form></div>;
}

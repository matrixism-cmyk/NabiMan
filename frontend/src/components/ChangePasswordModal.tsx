import React, { useState } from 'react';
import { apiPost } from '../hooks/useApi';
import { useT } from '../i18n';

export default function ChangePasswordModal({ onClose }: { onClose: () => void }) {
  const { t } = useT();
  const [currentPw, setCurrentPw] = useState('');
  const [newPw, setNewPw] = useState('');
  const [confirmPw, setConfirmPw] = useState('');
  const [msg, setMsg] = useState('');
  const [success, setSuccess] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setMsg('');
    if (newPw !== confirmPw) { setMsg(t('app.passwordMismatch')); return; }
    if (newPw.length < 4) { setMsg(t('app.passwordTooShort')); return; }
    const res = await apiPost<string>('/api/auth/change-password', { current_password: currentPw, new_password: newPw });
    if (res.success) { setSuccess(true); setMsg(t('app.passwordChanged')); }
    else { setMsg(res.message || t('app.failed')); }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-content" onClick={e => e.stopPropagation()}>
        <h3>{t('app.changePassword')}</h3>
        <form onSubmit={handleSubmit}>
          <input type="password" placeholder={t('app.currentPassword')} value={currentPw} onChange={e => setCurrentPw(e.target.value)} required autoFocus />
          <input type="password" placeholder={t('app.newPassword')} value={newPw} onChange={e => setNewPw(e.target.value)} required />
          <input type="password" placeholder={t('app.confirmPassword')} value={confirmPw} onChange={e => setConfirmPw(e.target.value)} required />
          {msg && <div className={success ? 'login-success' : 'login-error'}>{msg}</div>}
          <div className="modal-actions">
            {!success && <button type="submit" className="btn btn-primary">{t('common.change')}</button>}
            <button type="button" className="btn btn-secondary" onClick={onClose}>{success ? t('common.close') : t('common.cancel')}</button>
          </div>
        </form>
      </div>
    </div>
  );
}

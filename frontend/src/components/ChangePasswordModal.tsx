import React, { useState } from 'react';
import { apiPost } from '../shared/api/useApi';
import { useT } from '../i18n';
import { passwordOk } from '../shared/passwordRule';
import Modal from '../shared/ui/Modal';

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
    if (!passwordOk(newPw)) { setMsg(t('app.passwordTooShort')); return; }
    const res = await apiPost<string>('/api/auth/change-password', { current_password: currentPw, new_password: newPw });
    if (res.success) { setSuccess(true); setMsg(t('app.passwordChanged')); }
    else { setMsg(res.message || t('app.failed')); }
  };

  return (
    <Modal onClose={onClose} title={t('app.changePassword')}>
      <form onSubmit={handleSubmit}>
        <input type="password" placeholder={t('app.currentPassword')} aria-label={t('app.currentPassword')} autoComplete="current-password"
          value={currentPw} onChange={e => setCurrentPw(e.target.value)} required />
        <input type="password" placeholder={t('app.newPassword')} aria-label={t('app.newPassword')} autoComplete="new-password"
          value={newPw} onChange={e => setNewPw(e.target.value)} required />
        <input type="password" placeholder={t('app.confirmPassword')} aria-label={t('app.confirmPassword')} autoComplete="new-password"
          value={confirmPw} onChange={e => setConfirmPw(e.target.value)} required />
        {msg && <div className={success ? 'login-success' : 'login-error'} role={success ? 'status' : 'alert'}>{msg}</div>}
        <div className="modal-actions">
          {!success && <button type="submit" className="btn btn-primary">{t('common.change')}</button>}
          <button type="button" className="btn btn-secondary" onClick={onClose}>{success ? t('common.close') : t('common.cancel')}</button>
        </div>
      </form>
    </Modal>
  );
}

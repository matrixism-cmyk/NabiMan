import React, { useState } from 'react';
import Modal from '../../shared/ui/Modal';
import { apiPost } from '../../shared/api/useApi';
import { useT } from '../../i18n';
import { ROLE_HINT, ROLE_LABEL } from './userModel';
import { passwordOk } from '../../shared/passwordRule';

/** Making an account directly, approved from the start. */
export default function NewUserForm({ onClose, onCreated }: { onClose: () => void; onCreated: (username: string) => void }) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [role, setRole] = useState('remote');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const { t } = useT();

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!/^[A-Za-z0-9_-]{2,32}$/.test(username)) { setMessage(t('users.new.badId')); return; }
    if (!passwordOk(password)) { setMessage(t('users.new.shortPassword')); return; }
    if (password !== confirm) { setMessage(t('users.passwordMismatch')); return; }
    setBusy(true);
    try {
      const res = await apiPost<string>('/api/users', { username, password, role });
      if (res.success) onCreated(username);
      else setMessage(res.message || t('users.new.failed'));
    } catch {
      setMessage(t('users.requestFailed'));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal onClose={onClose} title={t('users.new.title')} className="users-new">
      <form className="users-form" onSubmit={submit}>
        {message && <p role="alert" className="message">{message}</p>}
        <label>{t('users.new.id')}
          <input className="filter-input" value={username} onChange={e => setUsername(e.target.value)} autoComplete="off" required />
        </label>
        <label>{t('users.new.password')}
          <input className="filter-input" type="password" value={password} onChange={e => setPassword(e.target.value)} autoComplete="new-password" required />
        </label>
        <label>{t('users.confirmPassword')}
          <input className="filter-input" type="password" value={confirm} onChange={e => setConfirm(e.target.value)} autoComplete="new-password" required />
        </label>
        <fieldset className="users-roles">
          <legend>{t('users.new.level')}</legend>
          {['remote', 'viewer', 'admin'].map(r => (
            <label key={r} className={`users-role-choice ${role === r ? 'is-on' : ''}`}>
              <input type="radio" name="new-role" value={r} checked={role === r} onChange={() => setRole(r)} />
              <strong>{t(ROLE_LABEL[r])}</strong>
              <small>{t(ROLE_HINT[r])}</small>
            </label>
          ))}
        </fieldset>
        <p className="text-secondary">{t('users.new.note')}</p>
        <div className="btn-group">
          <button type="submit" className="btn btn-primary" disabled={busy}>{busy ? t('users.new.creating') : t('users.new.create')}</button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>{t('common.cancel')}</button>
        </div>
      </form>
    </Modal>
  );
}
